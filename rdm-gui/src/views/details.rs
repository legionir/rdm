//! Detail panel for the selected download — the GUI form of `rdm info`
//! (overview, chunk table, event log and the `--json` snapshot).

use egui::{Color32, RichText, Ui};
use rdm::models::{ChunkStatus, DownloadRecord};
use rdm::utils::human;

use crate::state::{DetailTab, GuiState, UiAction};
use crate::util;
use crate::views::download_list::{state_color, state_glyph};

/// Log-pane filters: label plus the levels it keeps.
const LOG_FILTERS: [(&str, &[&str]); 4] = [
    ("all", &["error", "warn", "info", "debug", "trace"]),
    ("info+", &["error", "warn", "info"]),
    ("warn+", &["error", "warn"]),
    ("errors", &["error"]),
];

pub fn show(ui: &mut Ui, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();

    ui.horizontal(|ui| {
        for tab in DetailTab::ALL {
            if ui
                .selectable_label(state.detail_tab == tab, tab.title())
                .clicked()
            {
                state.detail_tab = tab;
            }
        }
        ui.separator();
        if let Some(record) = state.selected_record() {
            ui.label(
                RichText::new(format!(
                    "{} {} · {}",
                    state_glyph(record.state),
                    record.state,
                    record.filename
                ))
                .color(state_color(record.state))
                .strong(),
            );
        }
    });
    ui.separator();

    let tab = state.detail_tab;
    if tab == DetailTab::Log {
        log_tab(ui, state, &mut actions);
        return actions;
    }

    let Some(record) = state.selected_record().cloned() else {
        ui.label(
            RichText::new("Select a download to inspect it.")
                .italics()
                .color(Color32::from_gray(150)),
        );
        return actions;
    };

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .id_salt("detail-scroll")
        .show(ui, |ui| match tab {
            DetailTab::Overview => overview(ui, &record, state, &mut actions),
            DetailTab::Chunks => chunks(ui, state),
            DetailTab::Events => events(ui, state),
            DetailTab::Json => json(ui, state, &mut actions),
            DetailTab::Log => {}
        });

    actions
}

fn overview(
    ui: &mut Ui,
    record: &DownloadRecord,
    state: &GuiState,
    actions: &mut Vec<UiAction>,
) {
    egui::Grid::new("detail-overview")
        .num_columns(2)
        .spacing([14.0, 4.0])
        .min_col_width(130.0)
        .striped(true)
        .show(ui, |ui| {
            let mut field = |ui: &mut Ui, key: &str, value: String| {
                ui.label(RichText::new(key).small().color(Color32::from_gray(150)));
                ui.label(RichText::new(value).monospace().small());
                ui.end_row();
            };
            field(ui, "id", record.public_id.clone());
            field(ui, "state", record.state.to_string());
            field(ui, "url", record.url.clone());
            if let Some(eff) = &record.effective_url {
                if eff != &record.url {
                    field(ui, "effective url", eff.clone());
                }
            }
            field(ui, "file", record.filename.clone());
            field(ui, "output", record.output_path.clone());
            field(ui, "chunks dir", record.chunk_dir.clone());
            field(
                ui,
                "size",
                record
                    .total_size
                    .map(|s| human::human_bytes(s.max(0) as u64))
                    .unwrap_or_else(|| "unknown".to_string()),
            );
            field(
                ui,
                "downloaded",
                human::human_bytes(record.downloaded_size.max(0) as u64),
            );
            field(ui, "speed", human::human_rate(state.rate_of(record.id)));
            field(ui, "connections", record.max_connections.to_string());
            field(ui, "retries", record.retries.to_string());
            field(ui, "accept ranges", record.accept_ranges.to_string());
            if let (Some(algo), Some(expected)) =
                (&record.checksum_algorithm, &record.checksum_expected)
            {
                field(ui, "checksum", format!("{algo}:{expected}"));
            }
            if let Some(ua) = &record.user_agent {
                field(ui, "user agent", ua.clone());
            }
            if let Some(err) = &record.error {
                ui.label(RichText::new("last error").small().color(Color32::from_gray(150)));
                ui.label(
                    RichText::new(err)
                        .monospace()
                        .small()
                        .color(Color32::from_rgb(239, 68, 68)),
                );
                ui.end_row();
            }
            field(ui, "created", util::format_timestamp(record.created_at));
            field(ui, "updated", util::format_timestamp(record.updated_at));
            if let Some(t) = record.started_at {
                field(ui, "started", util::format_timestamp(t));
            }
            if let Some(t) = record.finished_at {
                field(ui, "finished", util::format_timestamp(t));
            }
        });

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("Copy URL").clicked() {
            actions.push(UiAction::CopyToClipboard(record.url.clone()));
        }
        if ui.small_button("Copy output path").clicked() {
            actions.push(UiAction::CopyToClipboard(record.output_path.clone()));
        }
        if ui.small_button("Open folder").clicked() {
            actions.push(UiAction::OpenOutputFolder(record.id));
        }
        if ui.small_button("Copy CLI command").clicked() {
            actions.push(UiAction::CopyToClipboard(cli_command(record)));
        }
    });
}

/// Reconstruct the equivalent `rdm download …` invocation for this record.
fn cli_command(record: &DownloadRecord) -> String {
    let mut cmd = format!(
        "rdm download \"{}\" --output \"{}\" --connections {}",
        record.url, record.output_path, record.max_connections
    );
    if record.retries != 5 {
        cmd.push_str(&format!(" --retry {}", record.retries));
    }
    if let (Some(algo), Some(expected)) = (&record.checksum_algorithm, &record.checksum_expected) {
        cmd.push_str(&format!(" --checksum {algo}:{expected}"));
    }
    if let Some(ua) = &record.user_agent {
        cmd.push_str(&format!(" --user-agent \"{ua}\""));
    }
    cmd
}

fn chunks(ui: &mut Ui, state: &GuiState) {
    if state.chunks.is_empty() {
        ui.label(
            RichText::new("No chunk rows yet.")
                .italics()
                .color(Color32::from_gray(150)),
        );
        return;
    }
    egui::Grid::new("detail-chunks")
        .num_columns(7)
        .striped(true)
        .spacing([10.0, 3.0])
        .show(ui, |ui| {
            for head in ["#", "START", "END", "DONE", "PROGRESS", "STATUS", "ERROR"] {
                ui.label(RichText::new(head).small().strong());
            }
            ui.end_row();
            for chunk in &state.chunks {
                ui.label(RichText::new(chunk.idx.to_string()).monospace().small());
                ui.label(RichText::new(chunk.start.to_string()).monospace().small());
                ui.label(RichText::new(chunk.end.to_string()).monospace().small());
                ui.label(
                    RichText::new(human::human_bytes(chunk.downloaded.max(0) as u64))
                        .monospace()
                        .small(),
                );
                let len = chunk.len().max(1) as f32;
                ui.add(
                    egui::ProgressBar::new((chunk.downloaded.max(0) as f32 / len).clamp(0.0, 1.0))
                        .desired_width(120.0),
                );
                let color = match chunk.status {
                    ChunkStatus::Completed => Color32::from_rgb(34, 197, 94),
                    ChunkStatus::Active => Color32::from_rgb(59, 130, 246),
                    ChunkStatus::Failed => Color32::from_rgb(239, 68, 68),
                    ChunkStatus::Pending => Color32::from_gray(150),
                };
                ui.label(RichText::new(chunk.status.to_string()).small().color(color));
                ui.label(
                    RichText::new(chunk.error.clone().unwrap_or_default())
                        .small()
                        .color(Color32::from_rgb(239, 68, 68)),
                );
                ui.end_row();
            }
        });
}

fn events(ui: &mut Ui, state: &GuiState) {
    if state.events.is_empty() {
        ui.label(
            RichText::new("No events recorded for this download.")
                .italics()
                .color(Color32::from_gray(150)),
        );
        return;
    }
    for (level, message, ts) in &state.events {
        let color = match level.as_str() {
            "error" => Color32::from_rgb(239, 68, 68),
            "warn" => Color32::from_rgb(245, 158, 11),
            _ => Color32::from_gray(170),
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(util::format_timestamp(*ts))
                    .monospace()
                    .small()
                    .color(Color32::from_gray(130)),
            );
            ui.label(RichText::new(format!("[{level}]")).small().color(color));
            ui.label(RichText::new(message).small());
        });
    }
}

fn json(ui: &mut Ui, state: &GuiState, actions: &mut Vec<UiAction>) {
    if ui.small_button("Copy JSON").clicked() {
        actions.push(UiAction::CopyToClipboard(state.json.clone()));
    }
    ui.add_space(4.0);
    let mut text = state.json.clone();
    ui.add(
        egui::TextEdit::multiline(&mut text)
            .code_editor()
            .desired_width(f32::INFINITY)
            .desired_rows(18)
            .interactive(false),
    );
}

fn log_tab(ui: &mut Ui, state: &mut GuiState, actions: &mut Vec<UiAction>) {
    ui.horizontal(|ui| {
        ui.label("show:");
        egui::ComboBox::from_id_salt("log-pane-filter")
            .selected_text(LOG_FILTERS[state.log_filter.min(LOG_FILTERS.len() - 1)].0)
            .width(110.0)
            .show_ui(ui, |ui| {
                for (idx, (label, _)) in LOG_FILTERS.iter().enumerate() {
                    ui.selectable_value(&mut state.log_filter, idx, *label);
                }
            });
        ui.separator();
        if ui.small_button("Clear").clicked() {
            actions.push(UiAction::ClearLog);
        }
        if ui.small_button("Copy").clicked() {
            let text = state
                .log
                .iter()
                .map(|l| format!("{} [{}] {}", util::format_timestamp(l.at), l.level, l.text))
                .collect::<Vec<_>>()
                .join("\n");
            actions.push(UiAction::CopyToClipboard(text));
        }
    });
    ui.separator();
    let allowed = LOG_FILTERS[state.log_filter.min(LOG_FILTERS.len() - 1)].1;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .id_salt("log-scroll")
        .show(ui, |ui| {
            for line in state.log.iter().filter(|l| allowed.contains(&l.level)) {
                let color = match line.level {
                    "error" => Color32::from_rgb(239, 68, 68),
                    "warn" => Color32::from_rgb(245, 158, 11),
                    "debug" | "trace" => Color32::from_gray(130),
                    _ => Color32::from_gray(180),
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(util::format_timestamp(line.at))
                            .monospace()
                            .small()
                            .color(Color32::from_gray(120)),
                    );
                    ui.label(
                        RichText::new(format!("{:<5}", line.level))
                            .monospace()
                            .small()
                            .color(color),
                    );
                    ui.label(RichText::new(&line.text).small().color(color));
                });
            }
        });
}
