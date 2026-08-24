//! Bottom status bar (collapsed by default) plus the expandable Events /
//! App log box that appears above it.

use egui::{Color32, Layout, RichText, Ui};

use crate::state::{FooterPanel, GuiState, UiAction};
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
    ui.add_space(2.0);
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
        let color = if state.status_is_error {
            Color32::from_rgb(220, 38, 38)
        } else {
            ui.visuals().weak_text_color()
        };
        let avail = ui.available_width();
        ui.add_sized(
            [(avail - 260.0).max(80.0), 16.0],
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
    ui.add_space(2.0);
}

/// The expandable box above the status bar. Rendered only while one of the
/// two footer buttons has been pressed; the ✕ on the right closes it.
pub fn panel(ui: &mut Ui, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let Some(panel) = state.footer_panel else {
        return actions;
    };

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(panel.title()).strong());
        if panel == FooterPanel::Events {
            if let Some(record) = state.selected_record() {
                ui.label(
                    RichText::new(format!("· {}", record.filename))
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
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
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        for (idx, (label, _)) in LOG_FILTERS.iter().enumerate() {
                            ui.selectable_value(&mut state.log_filter, idx, *label);
                        }
                    });
            }
        });
    });
    ui.separator();
    ui.add_space(4.0);

    match panel {
        FooterPanel::Events => events_box(ui, state),
        FooterPanel::AppLog => log_box(ui, state),
    }

    actions
}

/// Events of the selected download, one wrapped line each.
fn events_box(ui: &mut Ui, state: &GuiState) {
    if state.selected.is_none() {
        hint(ui, "Select a download to see its events.");
        return;
    }
    if state.events.is_empty() {
        hint(ui, "No events recorded for this download.");
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .id_salt("events-scroll")
        .show(ui, |ui| {
            for (level, message, ts) in &state.events {
                let color = match level.as_str() {
                    "error" => Color32::from_rgb(220, 38, 38),
                    "warn" => Color32::from_rgb(217, 119, 6),
                    _ => ui.visuals().weak_text_color(),
                };
                let ts_text = util::format_timestamp(*ts);
                let level = level.clone();
                let message = message.clone();
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(ts_text)
                            .monospace()
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.label(RichText::new(format!("[{level}]")).small().color(color));
                    ui.label(RichText::new(message).small().color(color));
                });
            }
        });
}

/// Captured engine + UI log, one wrapped line each.
fn log_box(ui: &mut Ui, state: &GuiState) {
    if state.log.is_empty() {
        hint(ui, "Nothing logged yet.");
        return;
    }
    let allowed = LOG_FILTERS[state.log_filter.min(LOG_FILTERS.len() - 1)].1;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .id_salt("log-scroll")
        .show(ui, |ui| {
            for line in state.log.iter().filter(|l| allowed.contains(&l.level)) {
                let color = match line.level {
                    "error" => Color32::from_rgb(220, 38, 38),
                    "warn" => Color32::from_rgb(217, 119, 6),
                    "debug" | "trace" => ui.visuals().weak_text_color(),
                    _ => Color32::GRAY,
                };
                let ts = util::format_timestamp(line.at);
                let level = line.level;
                let text = line.text.clone();
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(ts)
                            .monospace()
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
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

fn hint(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .italics()
            .color(ui.visuals().weak_text_color()),
    );
}
