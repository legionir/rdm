//! The download table (`rdm list` with buttons).

use egui::{Color32, RichText, Ui};
use rdm::models::{DownloadRecord, DownloadState};
use rdm::utils::human;

use crate::state::{progress_of, GuiState, UiAction};
use crate::util;

pub fn state_color(state: DownloadState) -> Color32 {
    match state {
        DownloadState::Completed => Color32::from_rgb(34, 197, 94),
        DownloadState::Running => Color32::from_rgb(59, 130, 246),
        DownloadState::Merging => Color32::from_rgb(139, 92, 246),
        DownloadState::Queued => Color32::from_rgb(245, 158, 11),
        DownloadState::Paused => Color32::from_rgb(148, 163, 184),
        DownloadState::Interrupted => Color32::from_rgb(249, 115, 22),
        DownloadState::Failed => Color32::from_rgb(239, 68, 68),
        DownloadState::Cancelled => Color32::from_gray(120),
    }
}

pub fn state_glyph(state: DownloadState) -> &'static str {
    match state {
        DownloadState::Completed => "✔",
        DownloadState::Running => "▶",
        DownloadState::Merging => "⛃",
        DownloadState::Queued => "…",
        DownloadState::Paused => "⏸",
        DownloadState::Interrupted => "⚠",
        DownloadState::Failed => "✖",
        DownloadState::Cancelled => "⃠",
    }
}

pub fn show(ui: &mut Ui, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let selected = state.selected;
    let rows: Vec<DownloadRecord> = state.visible_rows().into_iter().cloned().collect();
    let rates: Vec<f64> = rows.iter().map(|r| state.rate_of(r.id)).collect();

    if rows.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(24.0);
            ui.label(
                RichText::new("No downloads match. Press “New download” to add one.")
                    .italics()
                    .color(Color32::from_gray(150)),
            );
            ui.add_space(24.0);
        });
        return actions;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("downloads-grid")
                .num_columns(7)
                .striped(true)
                .spacing([10.0, 8.0])
                .min_col_width(60.0)
                .show(ui, |ui| {
                    header(ui);
                    for (record, rate) in rows.iter().zip(rates.iter()) {
                        row(ui, record, *rate, selected == Some(record.id), &mut actions);
                        ui.end_row();
                    }
                });
        });

    actions
}

fn header(ui: &mut Ui) {
    let head = |ui: &mut Ui, text: &str| {
        ui.label(RichText::new(text).small().strong());
    };
    head(ui, "STATE");
    head(ui, "FILE");
    head(ui, "PROGRESS");
    head(ui, "SIZE");
    head(ui, "SPEED");
    head(ui, "ETA");
    head(ui, "ACTIONS");
    ui.end_row();
}

fn row(
    ui: &mut Ui,
    record: &DownloadRecord,
    rate: f64,
    is_selected: bool,
    actions: &mut Vec<UiAction>,
) {
    let color = state_color(record.state);

    if ui
        .selectable_label(
            is_selected,
            RichText::new(format!("{} {}", state_glyph(record.state), record.state))
                .color(color)
                .small(),
        )
        .clicked()
    {
        actions.push(UiAction::Select(record.id));
    }

    ui.vertical(|ui| {
        let name = ui.label(RichText::new(&record.filename).strong());
        if name
            .on_hover_text(format!("{}\n{}", record.url, record.output_path))
            .clicked()
        {
            actions.push(UiAction::Select(record.id));
        }
        ui.label(
            RichText::new(format!(
                "{} · {} conns · added {}",
                record.public_id,
                record.max_connections,
                util::format_relative(record.created_at)
            ))
            .small()
            .color(Color32::from_gray(150)),
        );
    });

    let fraction = progress_of(record);
    ui.add(
        egui::ProgressBar::new(fraction)
            .desired_width(180.0)
            .text(format!("{:.1}%", fraction * 100.0)),
    );

    let total = record
        .total_size
        .map(|s| human::human_bytes(s.max(0) as u64))
        .unwrap_or_else(|| "?".to_string());
    ui.label(
        RichText::new(format!(
            "{} / {}",
            human::human_bytes(record.downloaded_size.max(0) as u64),
            total
        ))
        .monospace()
        .small(),
    );

    ui.label(
        RichText::new(if rate > 1.0 {
            human::human_rate(rate)
        } else {
            "—".to_string()
        })
        .monospace()
        .small(),
    );

    let eta = match (record.total_size, rate) {
        (Some(total), r) if total > 0 && r > 1.0 && record.state.active() => {
            let remaining = (total - record.downloaded_size).max(0) as f64;
            util::format_duration(remaining / r)
        }
        _ => "—".to_string(),
    };
    ui.label(RichText::new(eta).monospace().small());

    ui.horizontal(|ui| {
        let running = record.state.active();
        let resumable = matches!(
            record.state,
            DownloadState::Paused
                | DownloadState::Interrupted
                | DownloadState::Failed
                | DownloadState::Cancelled
                | DownloadState::Queued
                | DownloadState::Running
        ) && record.state != DownloadState::Completed;

        if running {
            if ui
                .small_button("⏸")
                .on_hover_text("Pause (rdm pause)")
                .clicked()
            {
                actions.push(UiAction::Pause(record.id));
            }
            if ui
                .small_button("⏹")
                .on_hover_text("Cancel (rdm cancel)")
                .clicked()
            {
                actions.push(UiAction::Cancel(record.id));
            }
        } else if resumable {
            if ui
                .small_button("▶")
                .on_hover_text("Resume (rdm resume)")
                .clicked()
            {
                actions.push(UiAction::Resume(record.id));
            }
        }

        if !running {
            if ui
                .small_button("⟲")
                .on_hover_text("Restart from scratch (rdm download --force)")
                .clicked()
            {
                actions.push(UiAction::Restart(record.id));
            }
        }
        if ui
            .small_button("📂")
            .on_hover_text("Open the containing folder")
            .clicked()
        {
            actions.push(UiAction::OpenOutputFolder(record.id));
        }
        if ui
            .small_button("🗑")
            .on_hover_text("Remove record (rdm remove)")
            .clicked()
        {
            actions.push(UiAction::AskRemove(record.id));
        }
    });
}
