//! Top toolbar: add, bulk controls, search, state filter and the sidebar
//! toggles for Queue and Settings.
//!
//! Layout rules (see the design spec):
//! * row 1 carries the actions; the sidebar toggles stay pinned right,
//! * row 2 carries search + filters; on [`LayoutMode::Compact`] windows the
//!   hint text is dropped so the row never wraps.

use egui::{Align, Layout, RichText, Ui};

use crate::state::{GuiState, UiAction, ALL_STATES};
use crate::theme::{self, components, Breakpoints, Icon, LayoutMode, Sizes, Spacing};
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
        if components::icon_text_button(
            ui,
            Icon::Plus,
            "New download",
            false,
            ux::new_download_tooltip(state.prefill_from_clipboard),
        )
        .clicked()
        {
            actions.push(UiAction::OpenAddDialog);
        }
        ui.separator();
        if ui
            .button(BulkAction::PauseAll.to_string())
            .on_hover_text(ux::pause_all_tooltip())
            .clicked()
        {
            actions.push(UiAction::PauseAll);
        }
        if ui
            .button(BulkAction::ResumeAll.to_string())
            .on_hover_text(ux::resume_all_tooltip())
            .clicked()
        {
            actions.push(UiAction::ResumeAll);
        }
        if ui
            .button(BulkAction::RemoveCompleted.to_string())
            .on_hover_text(ux::remove_completed_tooltip())
            .clicked()
        {
            actions.push(UiAction::AskRemoveCompleted);
        }
        ui.separator();
        if ui
            .button("Refresh")
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
                RichText::new(format!("{queued} queued"))
                    .small()
                    .color(palette.warning),
            )
            .on_hover_text("Queued downloads wait for a free slot — open the Queue sidebar");
            if ui
                .button(BulkAction::DropQueue.to_string())
                .on_hover_text(ux::drop_queue_tooltip())
                .clicked()
            {
                actions.push(UiAction::AskDropQueue);
            }
        }

        // Sidebar toggles, pinned to the right edge.
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if components::icon_text_button(
                ui,
                Icon::Info,
                "Settings",
                state.show_settings,
                "Open the settings window",
            )
            .clicked()
            {
                state.show_settings = !state.show_settings;
            }
            if components::icon_text_button(
                ui,
                Icon::Queue,
                "Queue",
                state.show_queue,
                "Toggle the queue sidebar",
            )
            .clicked()
            {
                state.show_queue = !state.show_queue;
            }
        });
    });

    ui.add_space(spacing.sm);

    ui.horizontal_wrapped(|ui| {
        ui.label("Search:");
        let search = components::text_edit(
            ui,
            &mut state.filter_text,
            sizes.search_width,
            "file, id or url",
        );
        // `Ctrl+F` is handled by the app loop, which raises this flag.
        if std::mem::take(&mut state.focus_search) {
            search.request_focus();
        }
        if components::icon_button(ui, Icon::Close, "Clear the search").clicked() {
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
                "Enter: details · Up/Down: select · Ctrl+F: search · F1: help",
            );
            if components::icon_button(
                ui,
                Icon::Info,
                "Help — states, shortcuts and vocabulary (F1)",
            ).clicked() {
                actions.push(UiAction::ToggleHelp);
            }
        }
    });

    actions
}
