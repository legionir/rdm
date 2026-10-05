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

use std::sync::{Arc, Mutex};

use egui::Context;
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

/// The live tray icon. Dropping it removes the icon, so the app owns it.
pub struct Tray {
    _icon: TrayIcon,
    /// The menu owns every item; only the check-marked one is touched again.
    drop_target: MenuItem,
    /// Commands from the two relay threads, drained by `poll` once per frame.
    commands: Arc<Mutex<Vec<TrayCommand>>>,
}

impl Tray {
    /// Build the icon and menu. `None` when this session has no tray host.
    ///
    /// `ctx` is woken on every command: the window is usually *hidden* while
    /// the tray is in use, and a hidden window only runs a frame when someone
    /// asks it to (that is what makes *Show* and *New download* react at once
    /// instead of on the next periodic refresh).
    pub fn new(drop_target_shown: bool, ctx: Context) -> Option<Self> {
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

        // The two crate channels are consumed here rather than in the frame
        // loop: the relay threads translate an event into a `TrayCommand` the
        // moment it happens and wake the window, which is what makes the menu
        // work while the window is hidden in the tray. Both loops end when the
        // channels close (process exit).
        let commands: Arc<Mutex<Vec<TrayCommand>>> = Arc::new(Mutex::new(Vec::new()));

        let menu_ids = MenuIds {
            show: show.id().clone(),
            new_download: new_download.id().clone(),
            pause_all: pause_all.id().clone(),
            resume_all: resume_all.id().clone(),
            drop_target: drop_target.id().clone(),
            quit: quit.id().clone(),
        };
        let menu_commands = Arc::clone(&commands);
        let menu_ctx = ctx.clone();
        std::thread::spawn(move || {
            while let Ok(event) = MenuEvent::receiver().recv() {
                let command = menu_ids.command_for(&event.id);
                if let Some(command) = command {
                    push(&menu_commands, command);
                    menu_ctx.request_repaint_of(egui::ViewportId::ROOT);
                }
            }
        });

        let icon_commands = Arc::clone(&commands);
        std::thread::spawn(move || {
            while let Ok(event) = TrayIconEvent::receiver().recv() {
                // Any click on the icon brings the window back; the menu itself
                // is on right click (`with_menu_on_left_click(false)`), so this
                // is the restore gesture rather than a menu shortcut.
                if matches!(
                    event,
                    TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. }
                ) {
                    push(&icon_commands, TrayCommand::Show);
                    ctx.request_repaint_of(egui::ViewportId::ROOT);
                }
            }
        });

        Some(Tray {
            _icon: tray,
            drop_target,
            commands,
        })
    }

    /// Reflect the drop-target setting in the menu item.
    pub fn set_drop_target_shown(&self, shown: bool) {
        let text = if shown {
            "Floating drop target ✓"
        } else {
            "Floating drop target"
        };
        let _ = self.drop_target.set_text(text);
    }

    /// Commands since the last call, in the order they happened.
    pub fn poll(&self) -> Vec<TrayCommand> {
        match self.commands.lock() {
            Ok(mut queue) => std::mem::take(&mut *queue),
            Err(_) => Vec::new(),
        }
    }
}

/// The menu item ids, owned by the relay thread so it can map a click to a
/// command without touching the menu again.
struct MenuIds {
    show: tray_icon::menu::MenuId,
    new_download: tray_icon::menu::MenuId,
    pause_all: tray_icon::menu::MenuId,
    resume_all: tray_icon::menu::MenuId,
    drop_target: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

impl MenuIds {
    fn command_for(&self, id: &tray_icon::menu::MenuId) -> Option<TrayCommand> {
        if id == &self.show {
            Some(TrayCommand::Show)
        } else if id == &self.new_download {
            Some(TrayCommand::NewDownload)
        } else if id == &self.pause_all {
            Some(TrayCommand::PauseAll)
        } else if id == &self.resume_all {
            Some(TrayCommand::ResumeAll)
        } else if id == &self.drop_target {
            Some(TrayCommand::ToggleDropTarget)
        } else if id == &self.quit {
            Some(TrayCommand::Quit)
        } else {
            None
        }
    }
}

fn push(queue: &Arc<Mutex<Vec<TrayCommand>>>, command: TrayCommand) {
    if let Ok(mut queue) = queue.lock() {
        queue.push(command);
    }
}
