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
//! ## Why the events are read on their own threads
//!
//! Tray commands are applied inside `App::update`, and `App::update` only runs
//! when the main window paints. The round-3 report (“the window goes to the
//! tray and the whole app freezes: no menu item works and the window never comes
//! back”) was precisely that: the window had been blanked with
//! `ViewportCommand::Visible(false)`, Windows sends no `WM_PAINT` to a window
//! that is not visible, so no frame ever ran again and every queued command —
//! including the one that would have shown the window again — waited for a
//! frame that could never come.
//!
//! So this module no longer depends on frames for the two things that must never
//! be lost:
//!
//! 1. **receiving** a click — one relay thread per channel blocks on
//!    `MenuEvent::receiver()` / `TrayIconEvent::receiver()`, so a click is
//!    captured even if the UI thread never runs another frame;
//! 2. **bringing the window back** — for the commands that are *about* the
//!    window (`Show`, `New download`), and whenever [`crate::frames`] says the
//!    UI has stopped delivering frames at all, the relay thread restores the
//!    window through the OS itself (`windows::reveal_main_window`), which needs
//!    no frame and no `egui::Context`.
//!
//! Everything else is queued in the inbox (plain command values, no `egui`
//! types) and applied by the app on the next frame; the relay also asks for a
//! repaint so that frame comes immediately. Touching the native menu is still
//! restricted to the UI thread and to real changes: refreshing a menu item while
//! its menu is open is the one thing that can block the UI thread.

use std::sync::{Arc, Mutex};

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
    /// These two are the ones the relay thread brings back itself, so “show me”
    /// and “new download” work even if the UI thread is not running frames.
    pub fn needs_window(self) -> bool {
        matches!(self, Self::Show | Self::NewDownload)
    }
}

/// The call the relay threads use to wake the UI (`Context::request_repaint`).
pub type Wake = dyn Fn() + Send + Sync;

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
        if let Err(err) = std::thread::Builder::new()
            .name("rdm-tray-menu".to_string())
            .spawn(move || {
                while let Ok(event) = MenuEvent::receiver().recv() {
                    if let Some(command) = command_for(&menu_ids, &event.id) {
                        deliver(command, &menu_inbox, &menu_wake);
                    }
                }
            })
        {
            // The app still works: `poll()` reads the channel itself as a
            // fallback, it just cannot rescue a window whose frame loop died.
            tracing::warn!("tray menu relay thread did not start: {err}");
        }

        // Icon clicks (left click / double click restores the window).
        let icon_inbox = Arc::clone(&inbox);
        let icon_wake = Arc::clone(&wake);
        if let Err(err) = std::thread::Builder::new()
            .name("rdm-tray-icon".to_string())
            .spawn(move || {
                while let Ok(event) = TrayIconEvent::receiver().recv() {
                    if matches!(
                        event,
                        TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. }
                    ) {
                        deliver(TrayCommand::Show, &icon_inbox, &icon_wake);
                    }
                }
            })
        {
            tracing::warn!("tray icon relay thread did not start: {err}");
        }

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

/// Queue one command and make sure the app will see it.
///
/// Runs on a relay thread: it may touch the inbox, ask for a repaint and use the
/// OS — never the app.
fn deliver(command: TrayCommand, inbox: &Mutex<Vec<TrayCommand>>, wake: &Wake) {
    if let Ok(mut queue) = inbox.lock() {
        queue.push(command);
    }
    wake();

    // “Bring the window back” cannot wait for a frame — that is the whole
    // failure mode this module exists for. The same escape hatch is used when
    // the frame clock says the UI has stopped running frames altogether, so a
    // *Pause all* click is never silently dropped either. In a healthy session
    // the clock is fresh (the app repaints every 250 ms), so only the two
    // window commands do this.
    let stale = crate::frames::is_stale();
    if command.needs_window() || stale {
        if crate::windows::reveal_main_window() {
            if stale && !command.needs_window() {
                tracing::info!(
                    "tray: {command:?} — the window had stopped painting, so it was brought \
                     back to the screen to apply the command"
                );
            }
        } else if stale {
            tracing::warn!(
                "tray: {command:?} is waiting for a frame, and no window handle is known to \
                 bring the window back"
            );
        }
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
        let wake: Arc<Wake> = Arc::new(|| {});
        // `deliver` is what the relay threads call for every click.
        deliver(TrayCommand::Show, &inbox, &*wake);
        deliver(TrayCommand::PauseAll, &inbox, &*wake);
        deliver(TrayCommand::Quit, &inbox, &*wake);

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
    fn the_drop_target_label_says_its_state_in_words() {
        // Round 3: a native menu draws its text with the system font, so a
        // `✓` there is an empty box on a font without the glyph.
        assert_eq!(drop_target_label(true), "Floating drop target: on");
        assert_eq!(drop_target_label(false), "Floating drop target: off");
    }
}
