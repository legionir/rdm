//! Bottom status bar (collapsed by default) plus the expandable Events /
//! App log box that appears above it.

use egui::{Layout, RichText, Ui};

use crate::state::{FooterPanel, GuiState, UiAction};
use crate::theme::{self, components, Palette, Sizes, Spacing};
use crate::util;

/// Log-pane filters: label plus the levels it keeps.
const LOG_FILTERS: [(&str, &[&str]); 4] = [
    ("all", &["error", "warn", "info", "debug", "trace"]),
    ("info+", &["error", "warn", "info"]),
    ("warn+", &["error", "warn"]),
    ("errors", &["error"]),
];

/// The one-line status bar: status + record counters + the two buttons that
/// expand the box above it.
pub fn status_bar(ui: &mut Ui, state: &mut GuiState) {
    let palette = theme::palette_of(ui);
    let sizes = Sizes::default();
    let spacing = Spacing::default();
    ui.add_space(spacing.xxs);
    ui.horizontal(|ui| {
        let (done, waiting, running, failed, cancelled) = state.counts();
        let text = format!(
            "{} ({} record(s) · {} done · {} running · {} waiting · {} failed · {} cancelled)",
            state.status,
            state.downloads.len(),
            done,
            running,
            waiting,
            failed,
            cancelled
        );
        let color = palette.status_color(state.status_is_error);
        let avail = ui.available_width();
        ui.add_sized(
            [
                (avail - sizes.status_reserve).max(sizes.status_min),
                sizes.status_height,
            ],
            egui::Label::new(RichText::new(text).small().color(color)).truncate(),
        );
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("App Log").small())
                        .selected(state.footer_panel == Some(FooterPanel::AppLog)),
                )
                .on_hover_text("Show the application log")
                .clicked()
            {
                state.footer_panel = if state.footer_panel == Some(FooterPanel::AppLog) {
                    None
                } else {
                    Some(FooterPanel::AppLog)
                };
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("Events").small())
                        .selected(state.footer_panel == Some(FooterPanel::Events)),
                )
                .on_hover_text("Show the events of the selected download")
                .clicked()
            {
                state.footer_panel = if state.footer_panel == Some(FooterPanel::Events) {
                    None
                } else {
                    Some(FooterPanel::Events)
                };
            }
        });
    });
    ui.add_space(spacing.xxs);
}

/// The expandable box above the status bar. Rendered only while one of the
/// two footer buttons has been pressed; the ✕ on the right closes it.
pub fn panel(ui: &mut Ui, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let Some(panel) = state.footer_panel else {
        return actions;
    };

    let palette = theme::palette_of(ui);
    let sizes = Sizes::default();
    let spacing = Spacing::default();
    ui.add_space(spacing.xs);
    ui.horizontal(|ui| {
        components::section_title(ui, panel.title());
        if panel == FooterPanel::Events {
            if let Some(record) = state.selected_record() {
                components::hint(ui, &palette, format!("· {}", record.filename));
            }
        }
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("✕")
                .on_hover_text("Close")
                .clicked()
            {
                state.footer_panel = None;
            }
            if panel == FooterPanel::AppLog {
                if ui.small_button("Copy").clicked() {
                    let text = state
                        .log
                        .iter()
                        .map(|l| {
                            format!("{} [{}] {}", util::format_timestamp(l.at), l.level, l.text)
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    actions.push(UiAction::CopyToClipboard(text));
                }
                if ui.small_button("Clear").clicked() {
                    actions.push(UiAction::ClearLog);
                }
                egui::ComboBox::from_id_salt("log-pane-filter")
                    .selected_text(
                        LOG_FILTERS[state.log_filter.min(LOG_FILTERS.len() - 1)]
                            .0
                            .to_string(),
                    )
                    .width(sizes.filter_width)
                    .show_ui(ui, |ui| {
                        for (idx, (label, _)) in LOG_FILTERS.iter().enumerate() {
                            ui.selectable_value(&mut state.log_filter, idx, *label);
                        }
                    });
            }
        });
    });
    ui.separator();
    ui.add_space(spacing.xs);

    match panel {
        FooterPanel::Events => events_box(ui, state, &palette),
        FooterPanel::AppLog => log_box(ui, state, &palette),
    }

    actions
}

/// Events of the selected download, one wrapped line each.
fn events_box(ui: &mut Ui, state: &GuiState, palette: &Palette) {
    if state.selected.is_none() {
        components::hint(ui, palette, "Select a download to see its events.");
        return;
    }
    if state.events.is_empty() {
        components::hint(ui, palette, "No events recorded for this download.");
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .id_salt("events-scroll")
        .show(ui, |ui| {
            for (level, message, ts) in &state.events {
                let color = palette.log_color(level);
                let ts_text = util::format_timestamp(*ts);
                let level = level.clone();
                let message = message.clone();
                ui.horizontal_wrapped(|ui| {
                    components::meta(ui, palette, ts_text);
                    ui.label(RichText::new(format!("[{level}]")).small().color(color));
                    ui.label(RichText::new(message).small().color(color));
                });
            }
        });
}

/// Captured engine + UI log, one wrapped line each.
fn log_box(ui: &mut Ui, state: &GuiState, palette: &Palette) {
    if state.log.is_empty() {
        components::hint(ui, palette, "Nothing logged yet.");
        return;
    }
    let allowed = LOG_FILTERS[state.log_filter.min(LOG_FILTERS.len() - 1)].1;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .id_salt("log-scroll")
        .show(ui, |ui| {
            for line in state.log.iter().filter(|l| allowed.contains(&l.level)) {
                let color = palette.log_color(line.level);
                let ts = util::format_timestamp(line.at);
                let level = line.level;
                let text = line.text.clone();
                ui.horizontal_wrapped(|ui| {
                    components::meta(ui, palette, ts);
                    ui.label(
                        RichText::new(format!("{level:<5}"))
                            .monospace()
                            .small()
                            .color(color),
                    );
                    ui.label(RichText::new(text).small().color(color));
                });
            }
        });
}
