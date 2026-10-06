//! Small OS bridges that egui/eframe do not expose.
//!
//! Two things are needed from the platform:
//!
//! 1. *Where* the floating drop target should sit so it floats just above the
//!    clock area of the taskbar. On Windows that is the primary monitor's work
//!    area (the desktop rectangle that excludes the taskbar), reported in
//!    physical pixels and converted to egui points with the system DPI.
//!    Everywhere else the caller falls back to the monitor size from egui,
//!    minus a margin.
//! 2. A window handle that can bring the main window back **without the app
//!    running a frame** — the tray's escape hatch.
//!
//! ## Why the tray needs raw Win32
//!
//! `App::update` is only called when the main window paints, and Windows sends
//! no `WM_PAINT` to a window that is not visible (winit's own hidden helper
//! window is created with `WS_VISIBLE | WS_POPUP` plus the layered style for
//! exactly that reason). Blanking the window with `ViewportCommand::Visible(
//! false)` therefore stops the frame loop for good: the click that should bring
//! the window back is read inside a frame that can never come again. The round-3
//! freeze was exactly that.
//!
//! So the tray's relay thread owns the escape hatch: [`reveal_main_window`]
//! restores and foregrounds the window from *any* thread, with the OS alone.
//! The handle it needs is captured once at start-up ([`remember_main_window`]).

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

use std::sync::atomic::{AtomicIsize, Ordering};

/// The main window's native handle, remembered once at start-up.
///
/// `0` means “unknown”, which makes every helper below a no-op instead of a
/// random guess (a session without a captured handle still runs, it just cannot
/// be woken from another thread).
static MAIN_WINDOW: AtomicIsize = AtomicIsize::new(0);

/// Remember the main window. Called once, from the `eframe` start-up closure.
pub fn remember_main_window(hwnd: isize) {
    MAIN_WINDOW.store(hwnd, Ordering::Relaxed);
}

#[cfg(target_os = "windows")]
pub fn remember_main_window_from(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    if let Ok(handle) = cc.window_handle() {
        if let RawWindowHandle::Win32(win32) = handle.as_raw() {
            remember_main_window(win32.hwnd.get());
        }
    }
}

/// No window handles to remember where the tray cannot be woken this way, so
/// the handle stays “unknown” and the helpers above stay no-ops.
#[cfg(not(target_os = "windows"))]
pub fn remember_main_window_from(_cc: &eframe::CreationContext<'_>) {
    remember_main_window(0);
}

/// Put the main window “in the tray”: invisible to the user, out of the taskbar,
/// out of Alt-Tab, click-through — and **still painting**.
///
/// This is the whole point of the round-3 fix, and it is the technique winit
/// itself uses for the helper window it must keep receiving messages on: leave
/// `WS_VISIBLE` set (or Windows sends no `WM_PAINT` at all — see the module
/// docs) and hide the window from the user with the *layered* style instead.
/// Both alternatives stop the paint stream — blanking the window clears
/// `WS_VISIBLE`, minimizing leaves nothing on screen to paint — and with no
/// paint there is no `App::update`, which is exactly where tray commands are
/// applied. That was the round-3 freeze.
///
/// The window is deactivated as well (a minimize/restore pair, with the alpha
/// already at zero so nothing can flash), so an invisible window never holds the
/// keyboard.
///
/// Returns `false` when no handle was captured: the caller then says so instead
/// of pretending the window went to the tray.
#[cfg(target_os = "windows")]
pub fn hide_main_window() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow,
        GWL_EXSTYLE, LWA_ALPHA, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SWP_NOZORDER, SW_MINIMIZE, SW_SHOWNOACTIVATE, WS_EX_APPWINDOW, WS_EX_LAYERED,
        WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };
    let raw = MAIN_WINDOW.load(Ordering::Relaxed);
    if raw == 0 {
        return false;
    }
    let hwnd = raw as *mut core::ffi::c_void;
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let wanted = (style
            | WS_EX_LAYERED as isize
            | WS_EX_TRANSPARENT as isize
            | WS_EX_TOOLWINDOW as isize)
            & !(WS_EX_APPWINDOW as isize);
        if wanted != style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        // Alpha zero *before* anything is shown, so the minimize/restore pair
        // below cannot flash: the window is invisible from here on.
        SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA);
        // Give the keyboard back to whatever had it (a minimized window is not
        // the foreground window), then display the window again without
        // activating it: a *displayed* window is what keeps paint coming.
        ShowWindow(hwnd, SW_MINIMIZE);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    true
}

/// Bring the main window back: opaque, in the taskbar, in Alt-Tab, foreground.
///
/// The counterpart of [`hide_main_window`], and callable from *any* thread — the
/// tray's relay thread uses it so that “Show rdm” works even in a session where
/// no frame is running.
#[cfg(target_os = "windows")]
pub fn reveal_main_window() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetForegroundWindow, SetLayeredWindowAttributes, SetWindowLongPtrW,
        SetWindowPos, ShowWindow, GWL_EXSTYLE, LWA_ALPHA, SWP_FRAMECHANGED, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_RESTORE, SW_SHOW, WS_EX_LAYERED,
        WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };
    let raw = MAIN_WINDOW.load(Ordering::Relaxed);
    if raw == 0 {
        return false;
    }
    let hwnd = raw as *mut core::ffi::c_void;
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let wanted = style
            & !(WS_EX_LAYERED as isize | WS_EX_TRANSPARENT as isize | WS_EX_TOOLWINDOW as isize);
        if wanted != style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
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
        // `SW_RESTORE` covers a minimized window and `SW_SHOW` a merely hidden
        // one: the tray can leave the window in either state.
        ShowWindow(hwnd, SW_RESTORE);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
    }
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
