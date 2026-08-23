//! Top toolbar: add, bulk controls, search and state filter.

use egui::{RichText, Ui};

use crate::state::{GuiState, UiAction, ALL_STATES};

pub fn show(ui: &mut Ui, state: &mut GuiState, active_jobs: usize) -> Vec<UiAction> {
    let mut actions = Vec::new();

    ui.horizontal_wrapped(|ui| {
        if ui
            .button(RichText::new("➕  New download").strong())
            .on_hover_text("rdm download <URL> …")
            .clicked()
        {
            actions.push(UiAction::OpenAddDialog);
        }
        ui.separator();
        if ui
            .button("⏸ Pause all")
            .on_hover_text("rdm pause <ID> for every active transfer")
            .clicked()
        {
            actions.push(UiAction::PauseAll);
        }
        if ui
            .button("▶ Resume all")
            .on_hover_text("rdm resume <ID> for every paused/interrupted/failed transfer")
            .clicked()
        {
            actions.push(UiAction::ResumeAll);
        }
        if ui
            .button("🗑 Clear completed")
            .on_hover_text("rdm remove <ID> for every completed record")
            .clicked()
        {
            actions.push(UiAction::RemoveCompleted);
        }
        ui.separator();
        if ui.button("🔄 Refresh").clicked() {
            actions.push(UiAction::Refresh);
        }
        if active_jobs > 0 {
            ui.add(egui::Spinner::new().size(14.0));
            ui.label(format!("{active_jobs} running here"));
        }
    });

    ui.horizontal_wrapped(|ui| {
        ui.label("Search:");
        ui.add(
            egui::TextEdit::singleline(&mut state.filter_text)
                .hint_text("file, id or url")
                .desired_width(220.0),
        );
        if ui.small_button("✕").clicked() {
            state.filter_text.clear();
        }
        ui.separator();
        let label = match state.state_filter {
            None => "all states".to_string(),
            Some(s) => s.to_string(),
        };
        egui::ComboBox::from_id_salt("state-filter")
            .selected_text(label)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.state_filter, None, "all states");
                for s in ALL_STATES {
                    ui.selectable_value(&mut state.state_filter, Some(s), s.as_str());
                }
            });
        if state.state_filter.is_none() {
            ui.checkbox(&mut state.show_completed, "show completed");
        }
        ui.separator();
        let (done, waiting, running, failed, cancelled) = state.counts();
        ui.label(
            RichText::new(format!(
                "✔ {done}   ⏸ {waiting}   ▶ {running}   ✖ {failed}   ⃠ {cancelled}"
            ))
            .monospace(),
        );
    });

    actions
}
