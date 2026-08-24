//! Top toolbar: add, bulk controls, search, state filter and the sidebar
//! toggles for Queue and Settings.

use egui::{Align, Color32, Layout, RichText, Ui};

use crate::state::{GuiState, UiAction, ALL_STATES};

pub fn show(
    ui: &mut Ui,
    state: &mut GuiState,
    active_jobs: usize,
    queued: usize,
) -> Vec<UiAction> {
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
        if queued > 0 {
            ui.label(
                RichText::new(format!("⏳ {queued} queued"))
                    .small()
                    .color(Color32::from_rgb(217, 119, 6)),
            )
            .on_hover_text("Waiting for a free slot — open the Queue sidebar");
            if ui.small_button("Clear queue").clicked() {
                actions.push(UiAction::ClearQueue);
            }
        }

        // Sidebar toggles, pinned to the right edge.
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("⚙  Settings"))
                        .selected(state.show_settings),
                )
                .on_hover_text("Toggle the settings sidebar")
                .clicked()
            {
                state.show_settings = !state.show_settings;
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("☰  Queue"))
                        .selected(state.show_queue),
                )
                .on_hover_text("Toggle the queue sidebar")
                .clicked()
            {
                state.show_queue = !state.show_queue;
            }
        });
    });

    ui.add_space(6.0);

    ui.horizontal_wrapped(|ui| {
        ui.label("Search:");
        ui.add(
            egui::TextEdit::singleline(&mut state.filter_text)
                .hint_text("file, id or url")
                .desired_width(220.0)
                .margin(egui::Margin::symmetric(6.0, 4.0)),
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
        ui.label(
            RichText::new("Double-click a row for details")
                .small()
                .color(ui.visuals().weak_text_color()),
        );
    });

    actions
}
