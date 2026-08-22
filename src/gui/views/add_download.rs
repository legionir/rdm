//! Add new download link form.
use egui::{Ui};
use crate::gui::state::{GuiState, UiAction};

pub fn show(ui: &mut Ui, state: &mut GuiState) -> Option<UiAction> {
    let mut action = None;
    if !state.show_add { return None; }
    egui::Window::new("New Download").collapsible(false).resizable(false).show(ui.ctx(), |ui| {
        ui.set_min_width(320.0);
        ui.vertical_centered_justified(|ui| {
            ui.strong("Download URL"); ui.separator();
            ui.horizontal(|ui| {
                ui.label("URL:");
                let resp = ui.add_sized([300.0, 22.0], egui::TextEdit::singleline(&mut state.new_url).hint_text("https://example.com/file.zip"));
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Start").clicked() && !state.new_url.is_empty() {
                    action = Some(UiAction::AddDownload { url: state.new_url.clone() });
                    state.new_url.clear(); state.show_add = false;
                }
                if ui.button("Cancel").clicked() { state.show_add = false; }
            });
        });
    });
    action
}
