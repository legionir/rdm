//! System-tray icon and its menu.
//!
//! Behaviour (UX decisions, see `audits/ux-feature-pack-report.md`):
//!
//! * the menu carries the frequent actions — *New download* (immediately
//!   pre-filled from the clipboard), *Pause all*, *Resume all*, *Show rdm*,
//!   the floating drop-target toggle and *Quit*;
//! * with `close_to_tray` on, closing the window hides it instead of quitting,
//!   and the transfer keeps running;
//! * the icon comes from `icon::tray_icon()`, the same asset as the window and
//!   the executable;
//! * if the tray cannot be created (a session without a tray host — also the
//!   case in CI), the app keeps working and *falls back to a normal window
//!   close*, so it can never become unclosable.
//!
//! ## Why this is polled from the frame loop (and why that is safe)
//!
//! The crate delivers menu clicks on an event channel. This module reads that
//! channel with `try_recv` **on the main thread** and translates an event into
//! a [`TrayCommand`] — no relay thread, no `egui::Context` on another thread.
//!
//! The hidden window is kept awake from the app loop instead
//! (`ctx.request_repaint_after(250 ms)` whenever a tray exists), because a
//! window hidden in the tray only runs a frame when something asks it to. That
//! ordering matters: an earlier version woke the app from a background thread
//! and also refreshed the menu text **every frame**. Refreshing a native menu
//! item while its menu is open can block the UI thread, and a blocked thread is
//! exactly what “the tray menu does nothing and the app freezes” looks like. So
//! the rule here is: touch the native menu only when a value really changed,
//! and never from a background thread.

use std::cell::Cell;

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

/// Menu-item ids, owned by this struct so a click can be mapped to a command
/// without touching the menu again.
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
    /// The one item whose text reflects a setting (a ✓ suffix).
    drop_target: MenuItem,
    ids: Ids,
    drop_target_shown: Cell<bool>,
}

impl Tray {
    /// Build the icon and menu. `None` when this session has no tray host.
    pub fn new(drop_target_shown: bool) -> Option<Self> {
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

        Some(Tray {
            _icon: tray,
            drop_target,
            ids,
            drop_target_shown: Cell::new(drop_target_shown),
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
    pub fn poll(&self) -> Vec<TrayCommand> {
        let mut commands = Vec::new();

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if let Some(command) = self.command_for(&event.id) {
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

    fn command_for(&self, id: &MenuId) -> Option<TrayCommand> {
        if id == &self.ids.show {
            Some(TrayCommand::Show)
        } else if id == &self.ids.new_download {
            Some(TrayCommand::NewDownload)
        } else if id == &self.ids.pause_all {
            Some(TrayCommand::PauseAll)
        } else if id == &self.ids.resume_all {
            Some(TrayCommand::ResumeAll)
        } else if id == &self.ids.drop_target {
            Some(TrayCommand::ToggleDropTarget)
        } else if id == &self.ids.quit {
            Some(TrayCommand::Quit)
        } else {
            None
        }
    }
}

/// The drop-target entry carries a ✓ when it is on — one item, no second
/// toggle, and the same words as the Settings switch.
fn drop_target_label(shown: bool) -> &'static str {
    if shown {
        "Floating drop target ✓"
    } else {
        "Floating drop target"
    }
}
