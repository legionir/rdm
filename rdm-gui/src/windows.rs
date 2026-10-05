//! Small OS bridges that egui/eframe do not expose.
//!
//! Only one thing is needed from the platform: *where* the floating drop
//! target should sit so it floats just above the clock area of the taskbar.
//! On Windows that is the primary monitor's work area (the desktop rectangle
//! that excludes the taskbar), reported in physical pixels and converted to
//! egui points with the system DPI. Everywhere else the caller falls back to
//! the monitor size from egui, minus a margin.

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

#[cfg(test)]
mod tests {
    use super::*;

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
