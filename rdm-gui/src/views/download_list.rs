//! Download list view.

use crate::state::{GuiState, UiAction};
use egui::{Color32, RichText, Ui};
use rdm::models::DownloadState;

pub fn show(ui: &mut Ui, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();
    ui.horizontal(|ui| {
        ui.strong("Downloads");
        ui.separator();
        let counts = state.counts();
        ui.small(format!(
            "✓ {}  ⏸ {}  ▶ {}  ✗ {}  ? {}",
            counts.0, counts.1, counts.2, counts.3, counts.4
        ));
    });
    ui.horizontal(|ui| {
        ui.label("Filter:");
        ui.text_edit_singleline(&mut state.filter_text);
        if ui.small_button("✕").clicked() {
            state.filter_text.clear();
        }
        ui.separator();
        if ui.small_button("+ Add").clicked() {
            state.show_add = true;
        }
    });
    ui.separator();
    let selected = state.selected_id;
    let rows = state.filtered_downloads();
    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
        for record in &rows {
            ui.horizontal(|ui| {
                let (label, color) = match record.state {
                    DownloadState::Completed => ("✓ Completed", Color32::from_rgb(34, 197, 94)),
                    DownloadState::Running | DownloadState::Merging => {
                        ("▶ Running", Color32::from_rgb(59, 130, 246))
                    }
                    DownloadState::Queued => ("⏸ Queued", Color32::from_rgb(245, 158, 11)),
                    DownloadState::Paused => ("⏸ Paused", Color32::from_gray(156)),
                    DownloadState::Failed | DownloadState::Interrupted => {
                        ("✗ Failed", Color32::from_rgb(239, 68, 68))
                    }
                    DownloadState::Cancelled => ("✘ Cancelled", Color32::DARK_GRAY),
                };
                ui.colored_label(color, label);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&record.filename).strong());
                    let total = record.total_size.unwrap_or(0).max(0) as f64;
                    let done = record.downloaded_size.max(0) as f64;
                    let pct = if total > 0.0 { (done / total) * 100.0 } else { 0.0 };
                    ui.small(format!(
                        "{} · {} · {:.1}% · {}",
                        record.public_id,
                        record.url,
                        pct,
                        format_timestamp(record.created_at)
                    ));
                });
                let width = 80.0;
                let rect = ui.allocate_space(egui::vec2(width, 12.0)).1;
                let progress = if total_progress(record) > 0.0 {
                    total_progress(record)
                } else {
                    0.0
                };
                ui.painter().rect_filled(rect, 2.0, Color32::from_gray(40));
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(
                        rect.min,
                        rect.min + egui::vec2(rect.width() * progress, rect.height()),
                    ),
                    2.0,
                    Color32::from_rgb(34, 197, 94),
                );
                ui.separator();
                if record.state.resumable()
                    && !matches!(record.state, DownloadState::Running | DownloadState::Queued)
                {
                    if ui.small_button("Resume").clicked() {
                        actions.push(UiAction::Resume { id: record.id });
                    }
                }
                if matches!(record.state, DownloadState::Running | DownloadState::Queued) {
                    if ui.small_button("Pause").clicked() {
                        actions.push(UiAction::Pause { id: record.id });
                    }
                }
                if record.state.active() {
                    if ui.small_button("Stop").clicked() {
                        actions.push(UiAction::Stop { id: record.id });
                    }
                }
                if ui.small_button("Del").clicked() {
                    actions.push(UiAction::DeleteItem { id: record.id });
                }
                if selected == Some(record.id) {
                    ui.colored_label(Color32::LIGHT_BLUE, "◉");
                }
            });
            ui.separator();
        }
    });
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Delete Completed").clicked() {
            actions.push(UiAction::DeleteCompleted);
        }
        if let Some(msg) = &state.status_message {
            ui.colored_label(Color32::from_rgb(234, 179, 8), msg);
        }
    });
    actions
}

fn total_progress(record: &rdm::models::DownloadRecord) -> f32 {
    let total = record.total_size.unwrap_or(0).max(0) as f32;
    if total > 0.0 {
        record.downloaded_size.max(0) as f32 / total
    } else {
        0.0
    }
}

fn format_timestamp(ts: i64) -> String {
    if ts == 0 {
        "—".into()
    } else {
        format!("{ts} sec")
    }
}
