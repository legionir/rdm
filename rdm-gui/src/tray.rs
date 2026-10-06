//! System-tray icon and its menu.
//!
//! Behaviour (UX decisions, see `audits/ux-feature-pack-report.md`):
//!
//! * the menu carries the frequent actions — *New download* (immediately
//!   pre-filled from the clipboard), *Pause all*, *Resume all*, *Show rdm*,
//!   the floating drop-target toggle and *Quit*;
//! * with `close_to_tray` on, closing the window puts it in the tray instead of
//!   quitting, and the transfer keeps running;
//! * the icon comes from `icon::tray_icon()`, the same asset as the window and
//!   the executable;
//! * if the tray cannot be created (a session without a tray host — also the
//!   case in CI), the app keeps working and *falls back to a normal window
//!   close*, so it can never become unclosable.
//!
//! ## Why the tray must not need a frame (round 3, second attempt)
//!
//! The round-3 report was “the window goes to the tray and the whole app
//! freezes: no menu item works and the window never comes back — the Task
//! Manager is the only way out”. The first fix tried to keep the frame loop
//! alive while the window was hidden, and the report came back unchanged, so
//! this module no longer *assumes* anything about painting:
//!
//! * the menu itself is drawn by the OS on the app's message pump (the tray
//!   window belongs to the thread that created the icon), which is why an
//!   otherwise dead app still shows a menu that opens and looks alive;
//! * the commands are read on two relay threads that block on
//!   `MenuEvent::receiver()` / `TrayIconEvent::receiver()` — a *global* channel
//!   in `muda`, so a click is captured even if the UI thread never runs another
//!   frame;
//! * the commands that must never depend on a frame are carried out by the
//!   relay thread itself, with the OS alone: *Show* and *New download* restore
//!   the window ([`crate::windows::reveal_main_window`]), and *Quit* restores
//!   the window **and arms a last-resort exit** ([`FORCE_QUIT_MS`]) so the user
//!   can never be left with a process only the Task Manager can close;
//! * the rest (*Pause all*, *Resume all*, the drop-target toggle) are applied
//!   inside the app because they need its state — and if [`crate::frames`] says
//!   the UI has stopped delivering frames, the relay brings the window back
//!   first, because a window that is visible catches up while a dead one never
//!   will;
//! * the relay also asks for a repaint after every event, so a healthy app
//!   reacts within one frame.
//!
//! Touching the native menu is still restricted to the UI thread and to real
//! changes: refreshing a menu item while its menu is open is the one thing that
//! can block the UI thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::icon;

/// One command from the tray UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// Bring the window back (also the icon's own click/double-click).
    Show,
    /// Open the form, pre-filled from the clipboard when it holds a link.
    NewDownload,
    PauseAll,
    ResumeAll,
    /// Show or hide the floating drop target.
    ToggleDropTarget,
    /// Really exit, transfers included.
    Quit,
}

impl TrayCommand {
    /// Does this command need the window on screen to mean anything?
    ///
    /// These two are the ones the relay thread carries out itself, so “show me”
    /// and “new download” work even if the UI thread is not running frames.
    pub fn needs_window(self) -> bool {
        matches!(self, Self::Show | Self::NewDownload)
    }
}

/// The call the relay threads use to wake the UI (`Context::request_repaint`).
pub type Wake = dyn Fn() + Send + Sync;

/// How long the app gets to exit after a *Quit* click before the tray ends the
/// process itself (ms).
///
/// The graceful path pauses running downloads and stops the backend, which is
/// capped at five seconds (`backend::shutdown(Duration::from_secs(5))`). Taking
/// longer than this means the frame loop is not going to apply the command at
/// all — and the honest alternative to a process only the Task Manager can
/// close is to close it here. Running downloads resume from their chunk files
/// on the next start.
const FORCE_QUIT_MS: u64 = 8_000;

/// Menu-item ids, owned by this struct so a click can be mapped to a command
/// without touching the menu again.
#[derive(Clone)]
struct Ids {
    show: MenuId,
    new_download: MenuId,
    pause_all: MenuId,
    resume_all: MenuId,
    drop_target: MenuId,
    quit: MenuId,
}

/// The live tray icon. Dropping it removes the icon, so the app owns it.
pub struct Tray {
    _icon: TrayIcon,
    /// The one item whose text reflects a setting ("on"/"off").
    drop_target: MenuItem,
    ids: Ids,
    drop_target_shown: std::cell::Cell<bool>,
    /// Commands the relay threads captured.
    inbox: Arc<Mutex<Vec<TrayCommand>>>,
}

impl Tray {
    /// Build the icon and menu, and start the relay threads.
    ///
    /// `wake` is called from those threads after every event: it must only ask
    /// the UI for a repaint (that is safe from any thread). `None` when this
    /// session has no tray host.
    pub fn new(drop_target_shown: bool, wake: Arc<Wake>) -> Option<Self> {
        let show = MenuItem::new("Show rdm", true, None);
        let new_download = MenuItem::new("New download (from clipboard)", true, None);
        let pause_all = MenuItem::new("Pause all", true, None);
        let resume_all = MenuItem::new("Resume all", true, None);
        let drop_target = MenuItem::new(drop_target_label(drop_target_shown), true, None);
        let quit = MenuItem::new("Quit rdm", true, None);

        let menu = Menu::new();
        menu.append_items(&[
            &show,
            &new_download,
            &PredefinedMenuItem::separator(),
            &pause_all,
            &resume_all,
            &PredefinedMenuItem::separator(),
            &drop_target,
            &quit,
        ])
        .ok()?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("rdm — the download keeps running here")
            .with_icon(icon::tray_icon())
            // Left click restores the window; the menu is on right click.
            .with_menu_on_left_click(false)
            .build()
            .ok()?;

        let ids = Ids {
            show: show.id().clone(),
            new_download: new_download.id().clone(),
            pause_all: pause_all.id().clone(),
            resume_all: resume_all.id().clone(),
            drop_target: drop_target.id().clone(),
            quit: quit.id().clone(),
        };

        let inbox: Arc<Mutex<Vec<TrayCommand>>> = Arc::new(Mutex::new(Vec::new()));

        // Menu clicks. `recv()` blocks, which is the point: the relay must not
        // need a frame to notice the click.
        let menu_ids = ids.clone();
        let menu_inbox = Arc::clone(&inbox);
        let menu_wake = Arc::clone(&wake);
        let menu_armed = std::thread::Builder::new()
            .name("rdm-tray-menu".to_string())
            .spawn(move || {
                while let Ok(event) = MenuEvent::receiver().recv() {
                    if let Some(command) = command_for(&menu_ids, &event.id) {
                        deliver(command, &menu_inbox, &*menu_wake);
                    }
                }
            })
            .is_ok();

        // Icon clicks (left click / double click restores the window).
        let icon_inbox = Arc::clone(&inbox);
        let icon_wake = Arc::clone(&wake);
        let icon_armed = std::thread::Builder::new()
            .name("rdm-tray-icon".to_string())
            .spawn(move || {
                while let Ok(event) = TrayIconEvent::receiver().recv() {
                    if matches!(
                        event,
                        TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. }
                    ) {
                        deliver(TrayCommand::Show, &icon_inbox, &*icon_wake);
                    }
                }
            })
            .is_ok();

        // One line the user can read back later: whether the rescue path this
        // report depended on is actually armed in the running process.
        tracing::info!(
            "tray: icon and menu created; relays {} (menu) / {} (icon)",
            if menu_armed { "armed" } else { "NOT armed" },
            if icon_armed { "armed" } else { "NOT armed" }
        );

        Some(Tray {
            _icon: tray,
            drop_target,
            ids,
            drop_target_shown: std::cell::Cell::new(drop_target_shown),
            inbox,
        })
    }

    /// Reflect the drop-target setting in the menu item.
    ///
    /// Called from the frame loop, so it must be a no-op when nothing changed:
    /// rewriting a native menu item every frame can block the UI thread (see
    /// the module note).
    pub fn set_drop_target_shown(&self, shown: bool) {
        if self.drop_target_shown.get() == shown {
            return;
        }
        self.drop_target_shown.set(shown);
        let _ = self.drop_target.set_text(drop_target_label(shown));
    }

    /// Commands since the last call, in the order they happened.
    ///
    /// The relay threads fill the inbox; the direct `try_recv` calls are the
    /// fallback for a session where a relay thread did not start, and they also
    /// pick up anything that arrived between two frames.
    pub fn poll(&self) -> Vec<TrayCommand> {
        let mut commands = self
            .inbox
            .lock()
            .map(|mut queue| std::mem::take(&mut *queue))
            .unwrap_or_default();

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if let Some(command) = command_for(&self.ids, &event.id) {
                commands.push(command);
            }
        }

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if matches!(
                event,
                TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. }
            ) {
                commands.push(TrayCommand::Show);
            }
        }

        commands
    }
}

/// Queue one command for the app, wake it — and, for the commands that must
/// never wait for a frame, carry them out right here.
///
/// Runs on a relay thread: it may touch the inbox, ask for a repaint and use the
/// OS — never the app.
fn deliver(command: TrayCommand, inbox: &Mutex<Vec<TrayCommand>>, wake: &Wake) {
    queue(command, inbox);
    wake();
    act(command);
}

/// Hand one command to the app, in order.
///
/// Separate from [`act`] because the two halves are tested differently: the
/// queue is ordinary data, while acting spends the OS. (It is also what keeps
/// the ordering test from arming the Quit deadline, which ends the process.)
fn queue(command: TrayCommand, inbox: &Mutex<Vec<TrayCommand>>) {
    if let Ok(mut queue) = inbox.lock() {
        queue.push(command);
    }
}

/// Everything a relay thread can do about a command without the app: the
/// commands that must never depend on a frame, and the rescue for the rest.
fn act(command: TrayCommand) {
    match command {
        // “Show me” and “new download” cannot wait: they are what a user clicks
        // when nothing else works.
        TrayCommand::Show | TrayCommand::NewDownload => reveal(command),
        // Quit gets the same treatment plus a deadline (see `arm_force_quit`).
        TrayCommand::Quit => {
            reveal(command);
            arm_force_quit();
        }
        // These need the app's state, so the app applies them — but a click that
        // lands in a frame loop that is not running would otherwise vanish, and
        // “the menu does nothing” is the exact defect this module exists for.
        _ => {
            if crate::frames::is_stale() {
                reveal(command);
            }
        }
    }
}

/// Restore the window through the OS, and say so either way.
///
/// The log lines matter: a report of “the tray does nothing” has to be readable
/// against what the process tried (the app log, and the log file next to the
/// metadata database).
fn reveal(command: TrayCommand) {
    if crate::windows::reveal_main_window() {
        tracing::info!("tray: {command:?} — window restored by the tray relay");
    } else {
        tracing::warn!(
            "tray: {command:?} clicked, but no window handle is known — queued for the next \
             frame instead"
        );
    }
}

/// Arm the last resort for *Quit*: if the app is still running after
/// [`FORCE_QUIT_MS`], end the process.
///
/// Armed once, by a real Quit click, and only reached when the graceful path —
/// pause the transfers, stop the backend, close the window — did not get there
/// in time. Without it, a UI thread that stopped painting leaves the user with
/// a window that cannot be closed and a process that only the Task Manager can
/// end; with it, the tray’s *Quit* is always true to its name.
fn arm_force_quit() {
    static ARMED: AtomicBool = AtomicBool::new(false);
    if ARMED.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("rdm-tray-quit".to_string())
        .spawn(|| {
            std::thread::sleep(Duration::from_millis(FORCE_QUIT_MS));
            // Still here: the frame loop never applied the quit.
            tracing::warn!(
                "tray: Quit was not applied within {FORCE_QUIT_MS} ms — exiting anyway \
                 (running downloads resume from their chunk files)"
            );
            std::process::exit(0);
        });
    if let Err(err) = spawned {
        tracing::warn!("tray: could not arm the Quit fallback: {err}");
    }
}

fn command_for(ids: &Ids, id: &MenuId) -> Option<TrayCommand> {
    if id == &ids.show {
        Some(TrayCommand::Show)
    } else if id == &ids.new_download {
        Some(TrayCommand::NewDownload)
    } else if id == &ids.pause_all {
        Some(TrayCommand::PauseAll)
    } else if id == &ids.resume_all {
        Some(TrayCommand::ResumeAll)
    } else if id == &ids.drop_target {
        Some(TrayCommand::ToggleDropTarget)
    } else if id == &ids.quit {
        Some(TrayCommand::Quit)
    } else {
        None
    }
}

/// The drop-target entry says its state in words — one item, no second toggle,
/// and the same words as the Settings switch.
///
/// It used to end in a check-mark glyph, which a native menu renders with the
/// system font: on a font without `U+2713` that is an empty box — the same
/// defect class as the drawn icons in `theme::icons`.
fn drop_target_label(shown: bool) -> &'static str {
    if shown {
        "Floating drop target: on"
    } else {
        "Floating drop target: off"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> Ids {
        Ids {
            show: MenuId::new("show"),
            new_download: MenuId::new("new-download"),
            pause_all: MenuId::new("pause-all"),
            resume_all: MenuId::new("resume-all"),
            drop_target: MenuId::new("drop-target"),
            quit: MenuId::new("quit"),
        }
    }

    #[test]
    fn every_menu_item_maps_to_its_command() {
        let ids = ids();
        assert_eq!(command_for(&ids, &ids.show), Some(TrayCommand::Show));
        assert_eq!(
            command_for(&ids, &ids.new_download),
            Some(TrayCommand::NewDownload)
        );
        assert_eq!(command_for(&ids, &ids.pause_all), Some(TrayCommand::PauseAll));
        assert_eq!(
            command_for(&ids, &ids.resume_all),
            Some(TrayCommand::ResumeAll)
        );
        assert_eq!(
            command_for(&ids, &ids.drop_target),
            Some(TrayCommand::ToggleDropTarget)
        );
        assert_eq!(command_for(&ids, &ids.quit), Some(TrayCommand::Quit));
        // A separator or a future item must not be mistaken for a command.
        assert_eq!(command_for(&ids, &MenuId::new("something-else")), None);
    }

    #[test]
    fn only_the_window_commands_need_the_window() {
        // These two are the ones the relay thread restores the window for, so
        // the list is a contract, not a detail: making `PauseAll` “need the
        // window” would pop the window up on every pause click.
        assert!(TrayCommand::Show.needs_window());
        assert!(TrayCommand::NewDownload.needs_window());
        for command in [
            TrayCommand::PauseAll,
            TrayCommand::ResumeAll,
            TrayCommand::ToggleDropTarget,
            TrayCommand::Quit,
        ] {
            assert!(!command.needs_window(), "{command:?} must not need the window");
        }
    }

    #[test]
    fn the_inbox_keeps_order_and_is_taken_exactly_once() {
        let inbox: Mutex<Vec<TrayCommand>> = Mutex::new(Vec::new());
        // `queue` is the ordering half of what the relay threads call for every
        // click (`deliver` = queue + wake + act, and `act(Quit)` arms a deadline
        // that ends the process — not something a test should do).
        queue(TrayCommand::Show, &inbox);
        queue(TrayCommand::PauseAll, &inbox);
        queue(TrayCommand::Quit, &inbox);

        let taken = {
            let mut queue = inbox.lock().unwrap();
            std::mem::take(&mut *queue)
        };
        assert_eq!(
            taken,
            vec![TrayCommand::Show, TrayCommand::PauseAll, TrayCommand::Quit]
        );
        assert!(
            inbox.lock().unwrap().is_empty(),
            "commands must be handed over once, not twice"
        );
    }

    #[test]
    fn the_quit_fallback_waits_for_the_graceful_shutdown() {
        // `backend::shutdown` is capped at five seconds, and the app only needs
        // this net when that path never ran. Firing too early would kill a
        // perfectly healthy shutdown; firing far too late is what the user
        // experiences as “I had to use the Task Manager”.
        assert!(
            FORCE_QUIT_MS > 5_000,
            "the Quit fallback ({FORCE_QUIT_MS} ms) must not cut off the graceful shutdown"
        );
        assert!(
            FORCE_QUIT_MS <= 15_000,
            "waiting {FORCE_QUIT_MS} ms for Quit is longer than a user will wait"
        );
    }

    #[test]
    fn the_drop_target_label_says_its_state_in_words() {
        // Round 3: a native menu draws its text with the system font, so a
        // `✓` there is an empty box on a font without the glyph.
        assert_eq!(drop_target_label(true), "Floating drop target: on");
        assert_eq!(drop_target_label(false), "Floating drop target: off");
    }
}
