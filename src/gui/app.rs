//! Main egui App trait implementation.
use egui::{CentralPanel, Context, SidePanel};
use crate::gui::state::{GuiState, UiAction};
use crate::gui::settings::SettingsObserver;
use crate::gui::views::{download_list, add_download, settings_view};

pub struct RdmGuiApp {
    pub state: GuiState,
    pub settings_observer: SettingsObserver,
}

impl Default for RdmGuiApp {
    fn default() -> Self { Self { state: GuiState::new(), settings_observer: SettingsObserver::new(None) } }
}

impl eframe::App for RdmGuiApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if self.settings_observer.check_reload() { self.apply_action(UiAction::ReloadSettings); }
        SidePanel::right("settings").default_width(240.0).show(ctx, |ui| {
            ui.heading("⚙ Settings"); ui.separator();
            let actions = settings_view::show(ui, &mut self.settings_observer, &mut self.state);
            for a in actions { self.apply_action(a); }
        });
        CentralPanel::default().show(ctx, |ui| {
            ui.heading("rdm — Rust Download Manager (GUI)"); ui.separator();
            let actions = download_list::show(ui, &mut self.state);
            for a in actions { self.apply_action(a); }
            ui.separator();
            if self.state.show_add {
                if let Some(a) = add_download::show(ui, &mut self.state) { self.apply_action(a); }
            }
        });
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {}
}

impl RdmGuiApp {
    pub fn new() -> Self { Self::default() }
    fn apply_action(&mut self, action: UiAction) {
        match action {
            UiAction::AddDownload { url } => { self.state.status_message = Some(format!("Adding: {}", url)); }
            UiAction::DeleteItem { id } => { self.state.downloads.retain(|r| r.id != id); self.state.status_message = Some(format!("Deleted item {}", id)); }
            UiAction::DeleteCompleted => { self.state.downloads.retain(|r| r.state != crate::models::download::DownloadState::Completed); self.state.status_message = Some("Deleted completed".into()); }
            UiAction::Resume { id } => { self.state.status_message = Some(format!("Resume {}", id)); }
            UiAction::Pause { id } => { self.state.status_message = Some(format!("Pause {}", id)); }
            UiAction::Stop { id } => { self.state.status_message = Some(format!("Stop {}", id)); }
            UiAction::ReloadSettings => { self.state.status_message = Some("Settings reloaded".into()); }
        }
    }
}
