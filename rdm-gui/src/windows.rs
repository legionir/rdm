//! Small OS bridges that egui/eframe do not expose.
//!
//! Three things are needed from the platform:
//!
//! 1. *Where* the floating drop target should sit so it floats just above the
//!    clock area of the taskbar. On Windows that is the primary monitor's work
//!    area (the desktop rectangle that excludes the taskbar), reported in
//!    physical pixels and converted to egui points with the system DPI.
//!    Everywhere else the caller falls back to the monitor size from egui,
//!    minus a margin.
//! 2. A window handle that can bring the main window back **without the app
//!    running a frame** — the tray's escape hatch.
//! 3. Hiding the window for the tray in a way that does *not* stop it painting.
//!
//! ## Why hiding is done this way (round 3, second attempt)
//!
//! `App::update` only runs when the main window paints, and **Windows sends no
//! `WM_PAINT` to a window that is not visible to it**. So a tray hide that
//! blanks the window (`ViewportCommand::Visible(false)`, which clears
//! `WS_VISIBLE`) stops the frame loop for good: the menu is drawn by the OS and
//! looks alive, nothing behind it ever reacts, and the only way out is the Task
//! Manager. Minimizing the window lands in the same hole — a minimized window
//! has nothing on screen to paint — and the first version of this fix did call
//! `SW_MINIMIZE`, which is exactly the trap it was trying to avoid.
//!
//! The rule this module follows now:
//!
//! * **never** minimise and **never** clear `WS_VISIBLE`; take the window off
//!   the screen with the *layered* style instead — alpha 0, click-through,
//!   tool-window, dropped to the bottom of the z-order — which is the same
//!   technique winit uses for the helper window it must keep receiving messages
//!   on. The window keeps painting (one frame a second is enough to stay
//!   responsive) while being invisible and un-clickable;
//! * and, because “keeps painting” is a property of the desktop this cannot be
//!   assumed, the tray never *depends* on it: the relay threads in
//!   [`crate::tray`] apply the window commands themselves through
//!   [`reveal_main_window`], from any thread, with no frame at all.
//!
//! The handle those relays need is captured at start-up from the frame loop
//! (`remember_main_window_from`) and, if that ever fails, *found* by walking the
//! process's own top-level windows ([`search_main_window`]) — a stale or missing
//! handle is a refusal, never a guess at somebody else's window.

/// Bottom-right anchor for a window of `size_pts`, in egui points.
///
/// `None` means “this platform has no work-area query” — the caller then uses
/// the monitor size instead of guessing.
#[cfg(target_os = "windows")]
pub fn bottom_right_anchor(size_pts: [f32; 2], margin_pts: f32) -> Option<[f32; 2]> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA};

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // 0 == no update flags: read the value, do not broadcast anything.
    let ok = unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut rect as *mut _ as *mut _, 0) };
    if ok == 0 {
        return None;
    }
    let dpi = unsafe { GetDpiForSystem() } as f32;
    let scale = if dpi > 0.0 { dpi / 96.0 } else { 1.0 };
    let mut pos = [
        (rect.right as f32) / scale - size_pts[0] - margin_pts,
        (rect.bottom as f32) / scale - size_pts[1] - margin_pts,
    ];
    // A monitor to the left of the primary has negative coordinates; never
    // place the window off-screen on the top-left edge.
    pos[0] = pos[0].max(0.0);
    pos[1] = pos[1].max(0.0);
    Some(pos)
}

/// No work-area query on this platform.
#[cfg(not(target_os = "windows"))]
pub fn bottom_right_anchor(_size_pts: [f32; 2], _margin_pts: f32) -> Option<[f32; 2]> {
    None
}

// ---------------------------------------------------------------- window -----

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

/// The title eframe starts the main window with (`main.rs`,
/// `eframe::run_native("RDM", …)`). Every other window the app opens titles
/// itself `rdm — …`, which is what lets a search tell them apart.
///
/// (Windows-only machinery, plus the test that pins the ranking on every
/// platform.)
#[cfg(any(target_os = "windows", test))]
const MAIN_TITLE: &str = "RDM";

/// The main window's native handle. `0` means “unknown”, which makes every
/// helper below a refusal instead of a random guess.
static MAIN_WINDOW: AtomicIsize = AtomicIsize::new(0);

/// Is the window parked in the tray right now? Drives the repaint heartbeat:
/// a hidden window needs far fewer frames than a visible one.
static HIDDEN: AtomicBool = AtomicBool::new(false);

/// Remember the main window. Called from the frame loop, which is the one place
/// that certainly has the window.
pub fn remember_main_window(hwnd: isize) {
    MAIN_WINDOW.store(hwnd, Ordering::Relaxed);
}

/// Is a usable handle known? (Logged once at start-up, so a report about the
/// tray can be read against what the process actually had.)
pub fn main_window_is_known() -> bool {
    #[cfg(target_os = "windows")]
    {
        resolve_window().is_some()
    }
    #[cfg(not(target_os = "windows"))]
    {
        MAIN_WINDOW.load(Ordering::Relaxed) != 0
    }
}

/// Is the window hidden in the tray right now?
pub fn main_window_is_hidden() -> bool {
    HIDDEN.load(Ordering::Relaxed)
}

/// Capture the handle from anything that can report one: eframe's
/// `CreationContext` before the first frame, and `eframe::Frame` from inside
/// the frame loop (a second chance if the first one had nothing to give).
#[cfg(target_os = "windows")]
pub fn remember_main_window_from(source: &impl raw_window_handle::HasWindowHandle) {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    if let Ok(handle) = source.window_handle() {
        if let RawWindowHandle::Win32(win32) = handle.as_raw() {
            remember_main_window(win32.hwnd.get());
        }
    }
}

/// No window handles to remember where the tray cannot be woken this way, so
/// the handle stays “unknown” and the helpers above stay no-ops.
#[cfg(not(target_os = "windows"))]
pub fn remember_main_window_from(_source: &impl raw_window_handle::HasWindowHandle) {
    remember_main_window(0);
}

/// How promising a window title is.
///
/// `2` is the main window itself, `1` a plausible stand-in, `0` something that
/// is definitely not it: the floating drop target, the Settings window and the
/// Help window all title themselves `rdm — …`, and the tray and winit helper
/// windows have no title at all.
#[cfg(any(target_os = "windows", test))]
fn title_rank(title: &str) -> u8 {
    if title == MAIN_TITLE {
        2
    } else if title.is_empty() || title.starts_with("rdm") {
        0
    } else {
        1
    }
}

/// The main window's handle: the remembered one when it is still a window, and
/// otherwise a search of our own top-level windows.
#[cfg(target_os = "windows")]
fn resolve_window() -> Option<isize> {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;

    let cached = MAIN_WINDOW.load(Ordering::Relaxed);
    if cached != 0 {
        // A handle outlives the window it named: Windows recycles them, and
        // hiding or revealing somebody else's window would be a real desktop
        // accident. Validate before every use.
        if unsafe { IsWindow(cached) } != 0 {
            return Some(cached);
        }
        remember_main_window(0);
    }
    let found = unsafe { search_main_window() };
    if let Some(hwnd) = found {
        remember_main_window(hwnd);
    }
    found
}

/// Find our own main window, so the tray can still restore it when no handle
/// was ever captured.
///
/// Only top-level, unowned windows of *this* process are considered, and the
/// title decides the winner (see [`title_rank`]). This is the difference
/// between “the tray could not bring the window back” and a tray that always
/// has a way home.
#[cfg(target_os = "windows")]
unsafe fn search_main_window() -> Option<isize> {
    use windows_sys::Win32::Foundation::LPARAM;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowTextW, GetWindowThreadProcessId, GW_OWNER,
    };

    struct Best {
        pid: u32,
        hwnd: isize,
        rank: u8,
    }

    unsafe extern "system" fn visit(hwnd: isize, lparam: LPARAM) -> i32 {
        let best = &mut *(lparam as *mut Best);
        if best.rank == 2 {
            return 0; // nothing can beat the main window: stop the walk
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid != best.pid {
            return 1; // somebody else's window
        }
        if GetWindow(hwnd, GW_OWNER) != 0 {
            return 1; // an owned popup, not the main window
        }
        let mut buf = [0u16; 128];
        let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        let title = String::from_utf16_lossy(&buf[..len.max(0) as usize]);
        let rank = title_rank(&title);
        if rank > best.rank {
            best.rank = rank;
            best.hwnd = hwnd;
        }
        if best.rank == 2 {
            0
        } else {
            1
        }
    }

    let mut best = Best {
        pid: std::process::id(),
        hwnd: 0,
        rank: 0,
    };
    EnumWindows(Some(visit), &mut best as *mut Best as LPARAM);
    (best.hwnd != 0).then_some(best.hwnd)
}

/// Put the main window in the tray: invisible, click-through, out of the
/// taskbar and out of Alt-Tab — and **still painting**.
///
/// See the module docs for why nothing here minimizes or hides the window the
/// way Windows understands “hidden”. Returns `false` when the window cannot be
/// taken off the screen (no handle, or the layered style was refused): the
/// caller then says so and leaves the window open instead of pretending it went
/// to the tray.
#[cfg(target_os = "windows")]
pub fn hide_main_window() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindow, GetWindowLongPtrW, SetForegroundWindow,
        SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GW_HWNDNEXT,
        HWND_BOTTOM, LWA_ALPHA, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SWP_NOZORDER, WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        WS_EX_TRANSPARENT,
    };

    let Some(hwnd) = resolve_window() else {
        return false;
    };
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let wanted = (style
            | WS_EX_LAYERED as isize
            | WS_EX_TRANSPARENT as isize
            | WS_EX_TOOLWINDOW as isize
            | WS_EX_NOACTIVATE as isize)
            & !(WS_EX_APPWINDOW as isize);
        if wanted != style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
            SetWindowPos(
                hwnd,
                0,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        // Alpha zero before anything else can show the window: from here on it
        // is invisible, so no minimize/restore pair can flash on screen.
        if SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA) == 0 {
            // Refused: put the extended style back exactly as it was, so the
            // window cannot be left in the “layered, but no transparency set”
            // state — which draws nothing while the app reports that it is
            // still on screen.
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
            SetWindowPos(
                hwnd,
                0,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            return false;
        }
        // An invisible window must not keep the keyboard: hand the foreground to
        // whatever is below it. (`WS_EX_NOACTIVATE` means it never gets it again
        // until `reveal_main_window` removes the style.)
        if GetForegroundWindow() == hwnd {
            let next = GetWindow(hwnd, GW_HWNDNEXT);
            if next != 0 {
                SetForegroundWindow(next);
            }
        }
        // Last, and deliberately non-blocking: drop to the bottom of the
        // z-order, still leaving `WS_VISIBLE` set, which is what keeps
        // `WM_PAINT` coming and the app awake.
        SetWindowPos(
            hwnd,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
    HIDDEN.store(true, Ordering::Relaxed);
    true
}

/// Bring the main window back: opaque, in the taskbar, in Alt-Tab, foreground.
///
/// The counterpart of [`hide_main_window`], and callable from *any* thread —
/// the tray's relay thread uses it so that “Show rdm” works even in a session
/// where no frame is running.
#[cfg(target_os = "windows")]
pub fn reveal_main_window() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetForegroundWindow, SetLayeredWindowAttributes, SetWindowLongPtrW,
        SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_TOP, LWA_ALPHA, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_RESTORE, SW_SHOW,
        WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };

    let Some(hwnd) = resolve_window() else {
        return false;
    };
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let wanted = style
            & !(WS_EX_LAYERED as isize
                | WS_EX_TRANSPARENT as isize
                | WS_EX_TOOLWINDOW as isize
                | WS_EX_NOACTIVATE as isize);
        if wanted != style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
            SetWindowPos(
                hwnd,
                0,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        // Opaque again — belt and braces for a session where the style change
        // itself was refused.
        SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
        // `SW_RESTORE` also covers a window some earlier build left minimized,
        // and `SW_SHOW` a window that had `WS_VISIBLE` cleared.
        ShowWindow(hwnd, SW_RESTORE);
        ShowWindow(hwnd, SW_SHOW);
        // Raise it without activating (the OS decides about activation next),
        // so the window is on top even if `SetForegroundWindow` is refused.
        SetWindowPos(
            hwnd,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        SetForegroundWindow(hwnd);
    }
    HIDDEN.store(false, Ordering::Relaxed);
    true
}

/// Nothing to hide where the tray cannot be woken this way.
#[cfg(not(target_os = "windows"))]
pub fn hide_main_window() -> bool {
    false
}

/// No raw window handle on this platform, so the tray cannot wake the app
/// itself; the app still restores its own window on the next frame.
#[cfg(not(target_os = "windows"))]
pub fn reveal_main_window() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_handle_is_a_refusal_not_a_guess() {
        // A session that never captured a handle must not pretend to reveal a
        // window: the tray logs the refusal instead. (`0` is the “unknown”
        // sentinel; the real handle is only captured on Windows.)
        remember_main_window(0);
        assert!(!reveal_main_window());
        assert!(!hide_main_window());
    }

    #[test]
    fn hiding_never_minimizes_the_window() {
        // The freeze in the round-3 report came back after a fix that hid the
        // window with a minimize/restore pair: a minimized window has nothing
        // on screen to paint, so the frame loop dies exactly as it does with
        // `Visible(false)`. `SW_MINIMIZE` must not appear in the code at all.
        let source = include_str!("windows.rs");
        assert!(
            !source.contains("SW_MINIMIZE"),
            "minimizing the window stops WM_PAINT — and with it every tray command"
        );
        // Built from two halves so the test does not trip over its own text.
        let forbidden = format!("{}{}", "ViewportCommand::Minimized(", "true)");
        assert!(!source.contains(&forbidden), "same trap, other spelling");
    }

    #[test]
    fn only_the_main_window_title_wins_the_search() {
        // The window search is how the tray finds the window when no handle was
        // captured, and it must never pick one of the app's *other* windows:
        // hiding or focusing the drop target instead of the main window would
        // leave the user with a tray they cannot get out of.
        assert_eq!(title_rank("RDM"), 2);
        assert_eq!(title_rank("rdm — Settings"), 0);
        assert_eq!(title_rank("rdm — Help"), 0);
        assert_eq!(title_rank("rdm — drop a link on the mark"), 0);
        // Windows without a title (the tray window, winit's helpers) are not it.
        assert_eq!(title_rank(""), 0);
        // Anything else is only a stand-in, never preferred over "RDM".
        assert_eq!(title_rank("Some other window"), 1);
        assert!(title_rank("RDM") > title_rank("Some other window"));
        assert!(title_rank("Some other window") > title_rank("rdm — Help"));
    }

    #[test]
    fn the_anchor_is_either_absent_or_on_screen() {
        // The Windows branch is verified by CI on windows-latest; here we only
        // assert the contract the caller relies on.
        match bottom_right_anchor([200.0, 96.0], 12.0) {
            Some([x, y]) => {
                assert!(x >= 0.0 && y >= 0.0, "anchor {x},{y} must not be negative");
            }
            None => {} // non-Windows: caller falls back to the monitor size
        }
    }
}
