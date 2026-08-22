//! Settings view panel with hot reload display.
use egui::{Ui, Color32};
use crate::gui::state::{GuiState, UiAction};
use crate::gui::settings::{AppSettings, SettingsObserver};

pub fn show(ui: &mut Ui, observer: &mut SettingsObserver, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();
    ui.collapsing("⚙ Settings (hot reload)", |ui| {
        ui.separator(); ui.strong("Connection & Storage");
        ui.horizontal(|ui| { ui.label("Max connections:"); ui.add(egui::DragValue::new(&mut observer.settings_mut().max_connections).speed(1)); });
        ui.horizontal(|ui| { ui.label("Download dir:"); ui.text_edit_singleline(&mut observer.settings_mut().download_dir); });
        ui.horizontal(|ui| { ui.label("Chunk size MB:"); ui.add(egui::DragValue::new(&mut observer.settings_mut().chunk_size_mb).speed(1)); });
        ui.horizontal(|ui| { ui.label("Retry limit:"); ui.add(egui::DragValue::new(&mut observer.settings_mut().retry_limit).speed(1)); });
        ui.separator();
        if observer.check_reload() { actions.push(UiAction::ReloadSettings); state.status_message = Some("Settings reloaded (hot)".into()); }
        ui.small(format!("File: {} · Max conn: {}", observer.settings().download_dir, observer.settings().max_connections));
        ui.separator();
        if ui.button("Force Reload").clicked() { observer.load(); actions.push(UiAction::ReloadSettings); state.status_message = Some("Settings forced reload".into()); }
    });
    actions
}
