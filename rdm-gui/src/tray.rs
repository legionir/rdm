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
//! `tray-icon` delivers menu clicks on an event channel that the app polls from
//! its frame loop; a click on the icon itself (Windows/macOS) restores the
//! window.

use std::sync::mpsc::{channel, Receiver};

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
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

struct Items {
    show: MenuItem,
    new_download: MenuItem,
    pause_all: MenuItem,
    resume_all: MenuItem,
    drop_target: MenuItem,
    quit: MenuItem,
}

/// The live tray icon. Dropping it removes the icon, so the app owns it.
pub struct Tray {
    _icon: TrayIcon,
    items: Items,
    icon_events: Receiver<TrayIconEvent>,
}

impl Tray {
    /// Build the icon and menu. `None` when this session has no tray host.
    pub fn new(drop_target_shown: bool) -> Option<Self> {
        let show = MenuItem::new("Show rdm", true, None);
        let new_download = MenuItem::new("New download (from clipboard)", true, None);
        let pause_all = MenuItem::new("Pause all", true, None);
        let resume_all = MenuItem::new("Resume all", true, None);
        let drop_target = MenuItem::new("Floating drop target", true, None);
        if drop_target_shown {
            // A check mark communicates the current state without a second item.
            let _ = drop_target.set_text("Floating drop target ✓");
        }
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

        let icon = icon::tray_icon();
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("rdm — the download keeps running here")
            .with_icon(icon)
            // Left click restores the window; the menu is on right click.
            .with_menu_on_left_click(false)
            .build()
            .ok()?;

        let (sender, icon_events) = channel();
        std::thread::spawn(move || {
            // The channel closes when the app drops its receiver; the loop then
            // ends on its own.
            while let Ok(event) = TrayIconEvent::receiver().recv() {
                if sender.send(event).is_err() {
                    break;
                }
            }
        });

        Some(Tray {
            _icon: tray,
            items: Items {
                show,
                new_download,
                pause_all,
                resume_all,
                drop_target,
                quit,
            },
            icon_events,
        })
    }

    /// Reflect the drop-target setting in the menu item.
    pub fn set_drop_target_shown(&self, shown: bool) {
        let text = if shown {
            "Floating drop target ✓"
        } else {
            "Floating drop target"
        };
        let _ = self.items.drop_target.set_text(text);
    }

    /// Commands since the last call (menu clicks and icon clicks).
    pub fn poll(&self) -> Vec<TrayCommand> {
        let mut commands = Vec::new();

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = &event.id;
            let command = if id == self.items.show.id() {
                Some(TrayCommand::Show)
            } else if id == self.items.new_download.id() {
                Some(TrayCommand::NewDownload)
            } else if id == self.items.pause_all.id() {
                Some(TrayCommand::PauseAll)
            } else if id == self.items.resume_all.id() {
                Some(TrayCommand::ResumeAll)
            } else if id == self.items.drop_target.id() {
                Some(TrayCommand::ToggleDropTarget)
            } else if id == self.items.quit.id() {
                Some(TrayCommand::Quit)
            } else {
                None
            };
            if let Some(command) = command {
                commands.push(command);
            }
        }

        while let Ok(event) = self.icon_events.try_recv() {
            // Any click on the icon brings the window back; the menu itself is
            // on right click (`with_menu_on_left_click(false)`), so this is the
            // restore gesture rather than a menu shortcut.
            match event {
                TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. } => {
                    commands.push(TrayCommand::Show)
                }
                _ => {}
            }
        }

        commands
    }
}
