//! The floating mark's own OLE drop target. **Windows only.**
//!
//! ## Why the window's own drag-and-drop is not enough (round 6)
//!
//! A drag from a browser is not a file drop. The drop handler `winit` installs
//! on Windows accepts exactly one clipboard format — `CF_HDROP` — and refuses
//! everything else: `platform_impl/windows/drop_handler.rs` of winit 0.30
//! builds its `FORMATETC` with `cfFormat: CF_HDROP` and returns
//! `DROPEFFECT_NONE` when `IDataObject::GetData` answers `DV_E_FORMATETC`
//! (but not a file). Windows then never calls `Drop` at all: the cursor shows
//! “no drop”, no `WindowEvent::DroppedFile` is delivered — and the app's
//! clipboard fallback never runs either, because from the app's point of view
//! nothing happened. That is exactly the reported “I dropped a link on the
//! floating mark and nothing happened”: a link dragged from Chrome, Edge or
//! Firefox offers `CF_UNICODETEXT` (and `text/uri-list`), **not** `CF_HDROP`.
//!
//! So the mark registers its **own** `IDropTarget`, which accepts files *and*
//! text, and hands the raw payload to [`crate::dropzone`] — the module that
//! owns the “what does this mean / what do we say” policy. Linux and the BSDs
//! need none of this: their window managers hand a URI list to the toolkit,
//! so `dropzone` keeps using egui's input there (see `DropZone::ui`).
//!
//! ## What is hand-rolled here, and why
//!
//! `windows-sys` declares functions but no COM interfaces, so the vtable
//! layouts below are written out: three `IUnknown` slots, then `GetData`,
//! `GetDataHere`, `QueryGetData` for `IDataObject`, and `DragEnter`, `DragOver`,
//! `DragLeave`, `Drop` for `IDropTarget`. They mirror the ones winit itself
//! uses, and the unit tests at the bottom of this file pin their *size*, so a
//! field appearing or disappearing in the wrong place cannot slip through as a
//! crash at drag time.
//!
//! Two deliberate differences from winit's version:
//!
//! * `QueryInterface` answers for `IUnknown` and `IDropTarget`. winit's is
//!   `unimplemented!()`, i.e. a panic — and a panic inside a COM callback runs
//!   through `extern "system"`, which aborts the process. It also means a
//!   callback that panics is caught here instead of taking the app down.
//! * the medium is released the way the OLE contract says to
//!   (`ReleaseStgMedium`: `Release` on `pUnkForRelease`, else the heap block),
//!   not by guessing which `Drag*` function fits.

use std::ffi::c_void;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::dropzone::DropPayload;

// ------------------------------------------------------------- Win32 basics

type Hresult = i32;

const S_OK: Hresult = 0;
/// `E_NOINTERFACE`, returned for any interface we do not implement.
const E_NOINTERFACE: Hresult = 0x8000_4002u32 as i32;

/// A COM `GUID`, declared here so this module needs nothing from `windows-sys`
/// beyond the functions it calls. Layout is the documented one.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

/// `IID_IUnknown` — every COM interface has to answer for this one.
const IID_IUNKNOWN: Guid = Guid {
    data1: 0x0000_0000,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

/// `IID_IDropTarget` — the interface `RegisterDragDrop` hands out.
const IID_IDROPTARGET: Guid = Guid {
    data1: 0x0000_0122,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

/// Fixed Win32 clipboard-format numbers (they never change; `windows-sys`
/// exposes them through the `Ole` module's constants, which this file spells
/// out so that what the target accepts is readable in one place).
const CF_UNICODETEXT: u16 = 13;
const CF_HDROP: u16 = 15;
/// `TYMED_HGLOBAL` and `DVASPECT_CONTENT`: the medium and aspect we ask for.
const TYMED_HGLOBAL: u32 = 1;
const DVASPECT_CONTENT: u32 = 1;
const DROPEFFECT_COPY: u32 = 1;
const DROPEFFECT_NONE: u32 = 0;

/// `FORMATETC` — what to ask a data object for.
#[repr(C)]
struct FormatEtc {
    cf_format: u16,
    target_device: *mut c_void,
    aspect: u32,
    index: i32,
    tymed: u32,
}

/// `STGMEDIUM` — what comes back. `data` is the `HGLOBAL` for `TYMED_HGLOBAL`
/// (the OLE union holds pointers only, so one pointer field has the same layout).
#[repr(C)]
struct StgMedium {
    tymed: u32,
    data: *mut c_void,
    release: *mut c_void,
}

#[repr(C)]
struct IUnknownVtbl {
    query_interface: unsafe extern "system" fn(
        this: *mut c_void,
        riid: *const Guid,
        out: *mut *mut c_void,
    ) -> Hresult,
    add_ref: unsafe extern "system" fn(this: *mut c_void) -> u32,
    release: unsafe extern "system" fn(this: *mut c_void) -> u32,
}

#[repr(C)]
struct IDataObjectVtbl {
    parent: IUnknownVtbl,
    get_data: unsafe extern "system" fn(
        this: *mut c_void,
        format: *const FormatEtc,
        out: *mut StgMedium,
    ) -> Hresult,
    get_data_here: unsafe extern "system" fn(
        this: *mut c_void,
        format: *const FormatEtc,
        out: *mut StgMedium,
    ) -> Hresult,
    query_get_data:
        unsafe extern "system" fn(this: *mut c_void, format: *const FormatEtc) -> Hresult,
}

#[repr(C)]
struct IDropTargetVtbl {
    parent: IUnknownVtbl,
    drag_enter: unsafe extern "system" fn(
        this: *mut IDropTarget,
        data: *mut c_void,
        key_state: u32,
        point: *const c_void,
        effect: *mut u32,
    ) -> Hresult,
    drag_over: unsafe extern "system" fn(
        this: *mut IDropTarget,
        key_state: u32,
        point: *const c_void,
        effect: *mut u32,
    ) -> Hresult,
    drag_leave: unsafe extern "system" fn(this: *mut IDropTarget) -> Hresult,
    drop: unsafe extern "system" fn(
        this: *mut IDropTarget,
        data: *mut c_void,
        key_state: u32,
        point: *const c_void,
        effect: *mut u32,
    ) -> Hresult,
}

/// The COM object OLE holds. `interface` **must** be the first field: OLE is
/// handed a pointer to it and this module recovers the whole object from that
/// pointer (`offset_of` is asserted in the tests below).
#[repr(C)]
struct IDropTarget {
    vtbl: *const IDropTargetVtbl,
}

// ------------------------------------------------------------ the drop target

/// The callbacks the target uses. Both run on the UI thread — OLE marshals to
/// the thread that registered the target, and eframe pumps its messages there.
pub struct Sink {
    /// A drag that carries something readable entered (true) or left (false).
    pub hover: Box<dyn Fn(bool) + Send + Sync>,
    /// A drag was released over the mark.
    pub drop: Box<dyn Fn(DropPayload) + Send + Sync>,
}

/// `#[repr(C)]` is not decoration here: OLE is handed a pointer to
/// `Target::interface` and [`Target::from_interface`] recovers the whole object
/// from that pointer, so the interface *must* be at offset 0. Without `repr(C)`
/// the compiler is free to order the fields any way it likes — which is exactly
/// what the offset assertion in the tests below caught.
#[repr(C)]
struct Target {
    interface: IDropTarget,
    refcount: AtomicUsize,
    sink: Sink,
    /// Registered ids of the URI-ish formats; 0 when the system has no such
    /// format (then that format is simply never asked for).
    uri_list: u16,
    /// `UniformResourceLocatorW`, for sources that offer only that.
    url: u16,
    /// `HTML Format` and `text/html`: accepted, but never parsed — a link in
    /// HTML markup is not read here, the announced clipboard fallback is.
    html: [u16; 2],
    /// What `DragOver` should answer: set by `DragEnter`.
    acceptable: AtomicBool,
}

/// Install our drop target on `hwnd` (the floating mark's window).
///
/// `Ok(())` means OLE will call this module for every drag over that window.
/// `Err` carries the HRESULT for the log — the caller keeps working without a
/// drop target rather than pretending the mark accepts drops.
pub fn install(hwnd: isize, sink: Sink) -> Result<(), String> {
    use windows_sys::Win32::System::Ole::{OleInitialize, RegisterDragDrop, RevokeDragDrop};

    // The same call winit makes when it enables drag-and-drop for a window: the
    // message loop that carries the OLE callbacks needs it. `S_FALSE` (already
    // initialised on this thread) is fine — and unlike winit this never panics,
    // because a target that cannot be installed is a log line, not a crash.
    unsafe { OleInitialize(ptr::null()) };

    let uri_list = register_format("text/uri-list");
    let url = register_format("UniformResourceLocatorW");
    // Two more formats a link can hide in. They are only ever used to *accept*
    // a drag: a source that offers nothing but HTML would otherwise be refused
    // before a drop happens and the user would see the mark ignore them, so the
    // drag is taken and the clipboard fallback (announced) finishes the job.
    let html = register_format("HTML Format");
    let text_html = register_format("text/html");
    let target = Box::into_raw(Box::new(Target {
        interface: IDropTarget {
            vtbl: &TARGET_VTBL as *const IDropTargetVtbl,
        },
        refcount: AtomicUsize::new(1),
        sink,
        uri_list,
        url,
        html: [html, text_html],
        acceptable: AtomicBool::new(false),
    }));

    // The window was created with drag-and-drop enabled (the builder keeps the
    // flag for the non-Windows path), so winit owns the target right now. OLE
    // allows exactly one `IDropTarget` per window, and ours is the one that can
    // read a link — take the window over. winit releases its own object when
    // the window is destroyed, so nothing is leaked by revoking it.
    let result = unsafe {
        RevokeDragDrop(hwnd as _);
        RegisterDragDrop(hwnd as _, target as *mut c_void as _)
    };
    if result < 0 {
        // Nobody took it, so the box is still ours to free.
        unsafe { drop(Box::from_raw(target)) };
        return Err(format!("RegisterDragDrop failed (0x{:08X})", result as u32));
    }
    tracing::info!(
        "drop target: the mark accepts files, text and URIs (text/uri-list = {uri_list}, \
         UniformResourceLocatorW = {url}, HTML Format = {html}, text/html = {text_html})"
    );
    // The reference this object was created with is deliberately *kept*, the
    // way winit keeps its own handler: OLE releases its reference when the
    // window is destroyed, and holding ours means a late callback can never
    // meet freed memory. One small allocation per window, once a session.
    Ok(())
}

/// Register (or look up) a clipboard format id. `0` means the system has no
/// id for it, and the format is then never asked for.
fn register_format(name: &str) -> u16 {
    use windows_sys::Win32::System::DataExchange::RegisterClipboardFormatW;

    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let id = unsafe { RegisterClipboardFormatW(wide.as_ptr()) };
    id.min(u16::MAX as u32) as u16
}

// ----------------------------------------------------------------- the vtable

static TARGET_VTBL: IDropTargetVtbl = IDropTargetVtbl {
    parent: IUnknownVtbl {
        query_interface: Target::query_interface,
        add_ref: Target::add_ref,
        release: Target::release,
    },
    drag_enter: Target::drag_enter,
    drag_over: Target::drag_over,
    drag_leave: Target::drag_leave,
    drop: Target::drop,
};

impl Target {
    /// The object behind an interface pointer (the interface is field zero).
    unsafe fn from_interface<'a>(this: *mut IDropTarget) -> &'a Target {
        unsafe { &*(this as *const Target) }
    }

    /// Run a UI-facing callback, and keep a panic in it from crossing back into
    /// `extern "system"` (which would abort the process: no `Drop`, no log, and
    /// from the user's seat a marker that kills the app).
    fn guarded(what: &str, run: impl FnOnce()) {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)).is_err() {
            tracing::error!("drop target: the {what} callback panicked (the drag continues)");
        }
    }

    unsafe extern "system" fn query_interface(
        this: *mut c_void,
        riid: *const Guid,
        out: *mut *mut c_void,
    ) -> Hresult {
        if riid.is_null() || out.is_null() {
            return E_NOINTERFACE;
        }
        let asked = unsafe { *riid };
        if asked == IID_IUNKNOWN || asked == IID_IDROPTARGET {
            unsafe {
                *out = this;
                let _ = Target::add_ref(this);
            }
            S_OK
        } else {
            unsafe { *out = ptr::null_mut() };
            E_NOINTERFACE
        }
    }

    unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
        let target = unsafe { &*(this as *const Target) };
        (target.refcount.fetch_add(1, Ordering::Relaxed) + 1) as u32
    }

    unsafe extern "system" fn release(this: *mut c_void) -> u32 {
        let target = unsafe { &*(this as *const Target) };
        let left = target.refcount.fetch_sub(1, Ordering::Release) - 1;
        if left == 0 {
            std::sync::atomic::fence(Ordering::Acquire);
            // The last reference goes: free the object the way it was made.
            unsafe { drop(Box::from_raw(this as *mut Target)) };
        }
        left as u32
    }

    unsafe extern "system" fn drag_enter(
        this: *mut IDropTarget,
        data: *mut c_void,
        _key_state: u32,
        _point: *const c_void,
        effect: *mut u32,
    ) -> Hresult {
        let target = unsafe { Target::from_interface(this) };
        let readable = !data.is_null()
            && unsafe { offers_readable(data, target.uri_list, target.url, target.html) };
        target.acceptable.store(readable, Ordering::Relaxed);
        let sink = &target.sink;
        Self::guarded("hover", move || (sink.hover)(readable));
        unsafe { *effect = if readable { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
        S_OK
    }

    unsafe extern "system" fn drag_over(
        this: *mut IDropTarget,
        _key_state: u32,
        _point: *const c_void,
        effect: *mut u32,
    ) -> Hresult {
        let target = unsafe { Target::from_interface(this) };
        unsafe {
            *effect = if target.acceptable.load(Ordering::Relaxed) {
                DROPEFFECT_COPY
            } else {
                DROPEFFECT_NONE
            }
        };
        S_OK
    }

    unsafe extern "system" fn drag_leave(this: *mut IDropTarget) -> Hresult {
        let target = unsafe { Target::from_interface(this) };
        target.acceptable.store(false, Ordering::Relaxed);
        let sink = &target.sink;
        Self::guarded("hover", move || (sink.hover)(false));
        S_OK
    }

    unsafe extern "system" fn drop(
        this: *mut IDropTarget,
        data: *mut c_void,
        _key_state: u32,
        _point: *const c_void,
        effect: *mut u32,
    ) -> Hresult {
        let target = unsafe { Target::from_interface(this) };
        target.acceptable.store(false, Ordering::Relaxed);
        let payload = if data.is_null() {
            DropPayload::default()
        } else {
            unsafe { read_payload(data, target.uri_list, target.url) }
        };
        let sink = &target.sink;
        Self::guarded("drop", move || {
            (sink.hover)(false);
            (sink.drop)(payload);
        });
        unsafe { *effect = DROPEFFECT_COPY };
        S_OK
    }
}

// ---------------------------------------------------------------- data access

/// Does this data object offer anything the mark could turn into a link?
///
/// `html` is in the list on purpose even though nothing ever *reads* HTML: a
/// drag that offers only HTML must still be accepted, or Windows refuses it
/// before a drop can happen and the user watches the mark ignore them. What
/// arrives is then whatever else the source offers, and — failing that — the
/// announced clipboard fallback.
unsafe fn offers_readable(data: *mut c_void, uri_list: u16, url: u16, html: [u16; 2]) -> bool {
    let formats = [CF_HDROP, CF_UNICODETEXT, uri_list, url, html[0], html[1]];
    formats
        .into_iter()
        .filter(|format| *format != 0)
        .any(|format| unsafe { query_get_data(data, format) })
}

/// `IDataObject::QueryGetData` — “do you have this format?”, asked without
/// copying anything, which is what a drag-over has to be.
unsafe fn query_get_data(data: *mut c_void, format: u16) -> bool {
    let vtbl = unsafe { *(data as *const *const IDataObjectVtbl) };
    if vtbl.is_null() {
        return false;
    }
    let ask = FormatEtc {
        cf_format: format,
        target_device: ptr::null_mut(),
        aspect: DVASPECT_CONTENT,
        index: -1,
        tymed: TYMED_HGLOBAL,
    };
    let call = unsafe { (*vtbl).query_get_data };
    unsafe { call(data, &ask) >= 0 }
}

/// Ask for one format and hand back the locked bytes of its `HGLOBAL`.
unsafe fn with_medium<T>(
    data: *mut c_void,
    format: u16,
    read: impl FnOnce(*const u8, usize) -> T,
) -> Option<T> {
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    if format == 0 {
        return None;
    }
    let vtbl = unsafe { *(data as *const *const IDataObjectVtbl) };
    if vtbl.is_null() {
        return None;
    }
    let ask = FormatEtc {
        cf_format: format,
        target_device: ptr::null_mut(),
        aspect: DVASPECT_CONTENT,
        index: -1,
        tymed: TYMED_HGLOBAL,
    };
    let mut medium: StgMedium = unsafe { std::mem::zeroed() };
    let call = unsafe { (*vtbl).get_data };
    if unsafe { call(data, &ask, &mut medium) } < 0 {
        return None;
    }
    if medium.tymed != TYMED_HGLOBAL || medium.data.is_null() {
        unsafe { release_medium(&medium) };
        return None;
    }

    let size = unsafe { GlobalSize(medium.data as _) };
    let start = unsafe { GlobalLock(medium.data as _) };
    let value = if start.is_null() || size == 0 {
        None
    } else {
        Some(read(start as *const u8, size))
    };
    if !start.is_null() {
        unsafe { GlobalUnlock(medium.data as _) };
    }
    unsafe { release_medium(&medium) };
    value
}

/// Hand a medium back the way `ReleaseStgMedium` would: the owning object
/// releases it, or — when there is none — the heap block is freed.
///
/// `GlobalFree` is declared by `windows-sys` next to `HGLOBAL` itself (in
/// `Win32::Foundation`), unlike `GlobalLock`/`GlobalSize`/`GlobalUnlock`, which
/// live in `Win32::System::Memory` — hence the two paths below that look like a
/// mistake and are not.
unsafe fn release_medium(medium: &StgMedium) {
    use windows_sys::Win32::Foundation::GlobalFree;

    if !medium.release.is_null() {
        let vtbl = unsafe { *(medium.release as *const *const IUnknownVtbl) };
        if !vtbl.is_null() {
            let release = unsafe { (*vtbl).release };
            unsafe { release(medium.release) };
        }
    } else if !medium.data.is_null() {
        unsafe { GlobalFree(medium.data as _) };
    }
}

/// Read UTF-16 text (the terminator is not part of it). Empty text is `None`:
/// a drag that carries only whitespace is a drag with nothing in it.
fn utf16_text(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 2 {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    let text = String::from_utf16_lossy(&units);
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Read one text format out of the data object.
///
/// The text is decoded *inside* the lock: the bytes belong to the medium, which
/// is released as soon as [`with_medium`] returns.
unsafe fn read_text(data: *mut c_void, format: u16) -> Option<String> {
    let read = |bytes: *const u8, len: usize| {
        let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
        utf16_text(bytes)
    };
    unsafe { with_medium(data, format, read) }.flatten()
}

/// Read `CF_HDROP` — the real files.
///
/// `DragQueryFileW` wants the handle the medium holds, not a view of its bytes,
/// so this format does its own `GetData` round trip instead of going through
/// [`with_medium`].
unsafe fn read_files(data: *mut c_void) -> Vec<PathBuf> {
    use windows_sys::Win32::UI::Shell::DragQueryFileW;

    // The `ifile` value that asks `DragQueryFileW` for the file count, and a
    // cap that is far past anything a person drags onto a 29 pt mark.
    const LIST_COUNT: u32 = u32::MAX;
    const MAX_FILES: u32 = 1024;

    let vtbl = unsafe { *(data as *const *const IDataObjectVtbl) };
    if vtbl.is_null() {
        return Vec::new();
    }
    let ask = FormatEtc {
        cf_format: CF_HDROP,
        target_device: ptr::null_mut(),
        aspect: DVASPECT_CONTENT,
        index: -1,
        tymed: TYMED_HGLOBAL,
    };
    let mut medium: StgMedium = unsafe { std::mem::zeroed() };
    let call = unsafe { (*vtbl).get_data };
    if unsafe { call(data, &ask, &mut medium) } < 0 {
        return Vec::new();
    }
    if medium.tymed != TYMED_HGLOBAL || medium.data.is_null() {
        unsafe { release_medium(&medium) };
        return Vec::new();
    }

    let mut files = Vec::new();
    let hdrop = medium.data;
    // `LIST_COUNT` asks “how many files are in there?”; the cap only guards
    // against a source that answers nonsense.
    let count = unsafe { DragQueryFileW(hdrop as _, LIST_COUNT, ptr::null_mut(), 0) };
    for index in 0..count.min(MAX_FILES) {
        let len = unsafe { DragQueryFileW(hdrop as _, index, ptr::null_mut(), 0) } as usize;
        if len == 0 {
            continue;
        }
        let mut buffer = vec![0u16; len + 1];
        let written = unsafe {
            DragQueryFileW(hdrop as _, index, buffer.as_mut_ptr(), buffer.len() as u32)
        } as usize;
        if written == 0 {
            continue;
        }
        let path = std::ffi::OsString::from_wide(&buffer[..written.min(len)]);
        files.push(PathBuf::from(path));
    }
    unsafe { release_medium(&medium) };
    files
}

/// Everything the data object carries, in the order the app prefers it.
unsafe fn read_payload(data: *mut c_void, uri_list: u16, url: u16) -> DropPayload {
    let mut payload = DropPayload {
        files: unsafe { read_files(data) },
        text: unsafe { read_text(data, CF_UNICODETEXT) },
        uri_list: unsafe { read_text(data, uri_list) },
    };
    if payload.text.is_none() {
        // The OLE spelling of a link, for sources that offer nothing else.
        payload.text = unsafe { read_text(data, url) };
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_com_layouts_match_the_abi() {
        // A vtable is an array of pointers, and the interface is only correct
        // when every slot is in its documented place: `IUnknown` is three, and
        // `IDropTarget` adds `DragEnter`, `DragOver`, `DragLeave`, `Drop`.
        // A field added in the wrong place (or a slot dropped) would make OLE
        // call into the wrong function — a crash, not a bug report — so the
        // sizes are pinned here.
        let pointer = std::mem::size_of::<*const c_void>();
        assert_eq!(std::mem::size_of::<IUnknownVtbl>(), 3 * pointer);
        assert_eq!(std::mem::size_of::<IDropTargetVtbl>(), 7 * pointer);
        assert_eq!(std::mem::size_of::<IDataObjectVtbl>(), 6 * pointer);
        assert_eq!(std::mem::size_of::<Guid>(), 16);
        assert_eq!(std::mem::align_of::<FormatEtc>(), pointer);
        assert_eq!(std::mem::align_of::<StgMedium>(), pointer);

        // FORMATETC and STGMEDIUM are documented Win32 structures; these are
        // their sizes on the 64-bit pointer width the GUI ships on (a 32-bit
        // build is measured against the same field order, not the numbers).
        if pointer == 8 {
            assert_eq!(std::mem::size_of::<FormatEtc>(), 32);
            assert_eq!(std::mem::size_of::<StgMedium>(), 24);
        } else {
            assert_eq!(std::mem::size_of::<FormatEtc>(), 20);
            assert_eq!(std::mem::size_of::<StgMedium>(), 12);
        }

        // OLE is handed a pointer to `Target::interface` and this module
        // recovers the object from it: the interface must be the object's
        // first field, or every callback would read the wrong memory.
        assert_eq!(std::mem::offset_of!(Target, interface), 0);
    }

    #[test]
    fn the_interfaces_answer_for_themselves_only() {
        assert_eq!(IID_IUNKNOWN.data1, 0);
        assert_eq!(IID_IDROPTARGET.data1, 0x0122);
        assert_eq!(IID_IUNKNOWN.data4[7], 0x46);
        assert_ne!(IID_IUNKNOWN, IID_IDROPTARGET);
    }

    #[test]
    fn text_is_read_up_to_the_terminator() {
        let mut bytes = Vec::new();
        for unit in "https://example.com/a.zip".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0xDEADu16.to_le_bytes());
        assert_eq!(
            utf16_text(&bytes).as_deref(),
            Some("https://example.com/a.zip"),
            "the terminator ends the text; whatever follows is not ours"
        );
    }

    #[test]
    fn text_that_is_only_space_is_nothing() {
        // A drag that carries "   " would otherwise open the form with a blank
        // link and a status sentence about a link that is not there.
        let blank: Vec<u8> = "   \r\n".encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(utf16_text(&blank), None);
        assert_eq!(utf16_text(&[]), None);
        assert_eq!(utf16_text(&[0x41]), None, "a lone byte is not a UTF-16 unit");
    }

    #[test]
    fn a_payload_is_readable_when_anything_survived() {
        assert!(!DropPayload::default().is_readable());
        assert!(DropPayload {
            text: Some("https://example.com".to_string()),
            ..Default::default()
        }
        .is_readable());
        assert!(DropPayload {
            files: vec![PathBuf::from("C:\\tmp\\a.zip")],
            ..Default::default()
        }
        .is_readable());
        assert!(DropPayload {
            uri_list: Some("file:///tmp/a.zip".to_string()),
            ..Default::default()
        }
        .is_readable());
    }
}
