//! A clock for one question: *is the UI still running frames?*
//!
//! The tray menu is read inside `App::update`, and `App::update` only ever runs
//! when the main window is painted. A window that Windows does not consider
//! visible receives **no `WM_PAINT` at all** — that is not folklore, it is the
//! reason winit creates its own invisible helper window with `WS_VISIBLE` and
//! the layered style (see the comment in `src/platform_impl/windows/
//! event_loop.rs` of winit 0.30). So if the app blanks the window with
//! `ViewportCommand::Visible(false)`, the paint stream stops, `App::update`
//! stops with it, and a tray click waits forever for a frame that can never
//! come. That was the round-3 freeze.
//!
//! The tray relay thread keeps this clock running. If a command arrives that is
//! *about* the window, or if the clock says the UI has stopped delivering
//! frames altogether, the relay brings the window back with the OS itself — no
//! frame needed — so no menu item is ever silently dropped.
//!
//! One `AtomicU64` of milliseconds: readable from any thread, no locks, and
//! monotone enough for a one-second liveness window.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// After this long without a frame the UI counts as “not running”.
///
/// The app asks for a repaint every 250 ms while its window is on screen, and
/// once a second while it sits in the tray (`app::TRAY_HEARTBEAT_MS`,
/// `app::TRAY_HIDDEN_HEARTBEAT_MS`), so a healthy app — hidden or not — stays
/// comfortably inside this window.
pub const STALE_MS: u64 = 2_000;

static LAST_FRAME_MS: AtomicU64 = AtomicU64::new(0);

/// Called at the top of every `App::update`.
pub fn note_frame() {
    LAST_FRAME_MS.store(now_ms(), Ordering::Relaxed);
}

/// How long ago the last frame ran.
///
/// `None` while the app has not run a single frame yet (startup, before the
/// first paint) — the relay thread does not treat that as “stale”, or it would
/// fight the app for the window during start-up.
pub fn millis_since_last_frame() -> Option<u64> {
    let last = LAST_FRAME_MS.load(Ordering::Relaxed);
    if last == 0 {
        None
    } else {
        Some(now_ms().saturating_sub(last))
    }
}

/// `true` when the UI thread has stopped delivering frames (see [`STALE_MS`]).
pub fn is_stale() -> bool {
    millis_since_last_frame().is_some_and(|age| age >= STALE_MS)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_makes_the_clock_fresh_and_a_long_gap_makes_it_stale() {
        // Before the first frame the clock must not claim staleness: the relay
        // thread would otherwise reveal the window during start-up.
        // (Only the `None` case can be asserted here — other tests in this
        // binary may already have noted a frame.)
        if LAST_FRAME_MS.load(Ordering::Relaxed) == 0 {
            assert_eq!(millis_since_last_frame(), None);
            assert!(!is_stale());
        }

        note_frame();
        let age = millis_since_last_frame().expect("a frame was just noted");
        assert!(age < STALE_MS, "a fresh frame must not look stale (age {age} ms)");
        assert!(!is_stale(), "a fresh frame must not look stale");

        // A frame from the past, further back than the liveness window: this is
        // what a stopped UI thread looks like to the tray.
        LAST_FRAME_MS.store(now_ms().saturating_sub(STALE_MS + 250), Ordering::Relaxed);
        assert!(is_stale(), "an old frame must look stale");
        note_frame();
        assert!(!is_stale(), "noting a frame makes the clock fresh again");
    }

    #[test]
    fn the_liveness_window_is_longer_than_the_repaint_heartbeat() {
        // The app asks for a repaint every 250 ms on screen and every 1 000 ms
        // in the tray (`app::TRAY_HEARTBEAT_MS`, `app::TRAY_HIDDEN_HEARTBEAT_MS`):
        // the window has to be comfortably longer than the slower of the two, or
        // the relay would keep “rescuing” a perfectly healthy app.
        assert!(
            STALE_MS >= 1_500,
            "STALE_MS is {STALE_MS} ms — too close to the 1 s heartbeat a hidden window uses"
        );
    }
}
