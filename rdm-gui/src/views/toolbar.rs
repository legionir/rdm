//! Top toolbar: add, bulk controls, search, state filter and the sidebar
//! toggles for Queue and Settings.
//!
//! Layout rules (see the design spec):
//! * row 1 carries the actions; the sidebar toggles stay pinned right,
//! * row 2 carries search + filters; on [`LayoutMode::Compact`] windows the
//!   hint text is dropped so the row never wraps.

use egui::{Align, Layout, RichText, Ui};

use crate::state::{GuiState, UiAction, ALL_STATES};
use crate::theme::{self, components, Breakpoints, LayoutMode, Sizes, Spacing};
use crate::ux::{self, BulkAction};

pub fn show(
    ui: &mut Ui,
    state: &mut GuiState,
    active_jobs: usize,
    queued: usize,
) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let palette = theme::palette_of(ui);
    let sizes = Sizes::default();
    let spacing = Spacing::default();
    let mode = Breakpoints::default().mode(ui.available_width());

    ui.horizontal_wrapped(|ui| {
        if components::primary_button(ui, "➕  New download")
            .on_hover_text("rdm download <URL> …")
            .clicked()
        {
            actions.push(UiAction::OpenAddDialog);
        }
        ui.separator();
        if ui
            .button(format!("⏸ {}", BulkAction::PauseAll))
            .on_hover_text(ux::pause_all_tooltip())
            .clicked()
        {
            actions.push(UiAction::PauseAll);
        }
        if ui
            .button(format!("▶ {}", BulkAction::ResumeAll))
            .on_hover_text(ux::resume_all_tooltip())
            .clicked()
        {
            actions.push(UiAction::ResumeAll);
        }
        if ui
            .button(format!("🗑 {}", BulkAction::RemoveCompleted))
            .on_hover_text(ux::remove_completed_tooltip())
            .clicked()
        {
            actions.push(UiAction::AskRemoveCompleted);
        }
        ui.separator();
        if ui
            .button("🔄 Refresh")
            .on_hover_text("Re-read the metadata database (F5)")
            .clicked()
        {
            actions.push(UiAction::Refresh);
        }
        if active_jobs > 0 {
            ui.add(egui::Spinner::new().size(spacing.spinner));
            ui.label(format!("{active_jobs} running here"));
        }
        if queued > 0 {
            ui.label(
                RichText::new(format!("⏳ {queued} queued"))
                    .small()
                    .color(palette.warning),
            )
            .on_hover_text("Queued downloads wait for a free slot — open the Queue sidebar (☰)");
            if ui
                .small_button(format!("{}", BulkAction::DropQueue))
                .on_hover_text(ux::drop_queue_tooltip())
                .clicked()
            {
                actions.push(UiAction::DropQueue);
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
                    egui::Button::new(RichText::new("☰  Queue")).selected(state.show_queue),
                )
                .on_hover_text("Toggle the queue sidebar")
                .clicked()
            {
                state.show_queue = !state.show_queue;
            }
        });
    });

    ui.add_space(spacing.sm);

    ui.horizontal_wrapped(|ui| {
        ui.label("Search:");
        let search = ui.add(
            egui::TextEdit::singleline(&mut state.filter_text)
                .hint_text("file, id or url")
                .desired_width(sizes.search_width)
                .margin(egui::Margin::symmetric(spacing.sm, spacing.xs)),
        );
        // `Ctrl+F` is handled by the app loop, which raises this flag.
        if std::mem::take(&mut state.focus_search) {
            search.request_focus();
        }
        if components::icon_button(ui, "✕", "Clear the search") {
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
        if mode != LayoutMode::Compact {
            ui.separator();
            components::hint(
                ui,
                &palette,
                "Enter: details · ↑/↓: select · Ctrl+F: search",
            );
            ui.label(RichText::new("ⓘ").small().color(palette.text_muted))
                .on_hover_text(ux::legend_tooltip());
        }
    });

    actions
}
