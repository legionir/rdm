//! The download table (`rdm list` with buttons).
//!
//! Rows are full-width: clicking anywhere on a row selects it and
//! double-clicking opens the details modal.

use egui::{Align, Color32, Layout, RichText, Sense, Ui, UiBuilder};
use rdm::models::{DownloadRecord, DownloadState};
use rdm::utils::human;

use crate::state::{progress_of, GuiState, UiAction};
use crate::util;

pub fn state_color(ui: &Ui, state: DownloadState) -> Color32 {
    match state {
        DownloadState::Completed => Color32::from_rgb(22, 163, 74),
        DownloadState::Running => Color32::from_rgb(37, 99, 235),
        DownloadState::Merging => Color32::from_rgb(139, 92, 246),
        DownloadState::Queued => Color32::from_rgb(217, 119, 6),
        DownloadState::Paused => ui.visuals().weak_text_color(),
        DownloadState::Interrupted => Color32::from_rgb(234, 88, 12),
        DownloadState::Failed => Color32::from_rgb(220, 38, 38),
        DownloadState::Cancelled => ui.visuals().weak_text_color(),
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

/// Horizontal padding inside a row, and the gap between two columns.
const PAD: f32 = 8.0;
const GAP: f32 = 8.0;
/// Column widths adapt to the available width; `FILE` is the flexible one.
pub struct Columns {
    pub state: f32,
    pub file: f32,
    pub id: f32,
    pub conns: f32,
    pub added: f32,
    pub progress: f32,
    pub size: f32,
    pub speed: f32,
    pub eta: f32,
    pub actions: f32,
    pub show_added: bool,
    pub show_speed: bool,
    pub show_eta: bool,
}

impl Columns {
    fn for_width(width: f32) -> Self {
        let mut c = Columns {
            state: 82.0,
            file: 140.0,
            id: 84.0,
            conns: 78.0,
            added: 86.0,
            progress: 150.0,
            size: 106.0,
            speed: 72.0,
            eta: 58.0,
            actions: 158.0,
            show_added: true,
            show_speed: true,
            show_eta: true,
        };
        let fixed = |c: &Columns| {
            c.state + c.id + c.conns + c.progress + c.size + c.actions
                + if c.show_added { c.added } else { 0.0 }
                + if c.show_speed { c.speed } else { 0.0 }
                + if c.show_eta { c.eta } else { 0.0 }
        };
        // 7 columns are always shown (state, file, id, conns, progress, size,
        // actions); the three optional ones can be dropped on narrow panels.
        let cols = |c: &Columns| {
            7 + (c.show_added as usize) + (c.show_speed as usize) + (c.show_eta as usize)
        };
        let budget = |c: &Columns| width - 2.0 * PAD - (cols(c) as f32 - 1.0) * GAP - 140.0;
        if budget(&c) < fixed(&c) {
            c.show_added = false;
        }
        if budget(&c) < fixed(&c) {
            c.show_speed = false;
        }
        if budget(&c) < fixed(&c) {
            c.show_eta = false;
        }
        if budget(&c) < fixed(&c) {
            c.progress = 90.0;
        }
        // Still too narrow? Scale the fixed columns down proportionally so the
        // row never grows wider than the panel (FILE keeps at least 80 px).
        let avail_fixed = width - 2.0 * PAD - (cols(&c) as f32 - 1.0) * GAP - 80.0;
        let needed = fixed(&c);
        if avail_fixed < needed && needed > 0.0 {
            let scale = (avail_fixed / needed).clamp(0.25, 1.0);
            c.state *= scale;
            c.id *= scale;
            c.conns *= scale;
            c.progress *= scale;
            c.size *= scale;
            c.actions *= scale;
            if c.show_added {
                c.added *= scale;
            }
            if c.show_speed {
                c.speed *= scale;
            }
            if c.show_eta {
                c.eta *= scale;
            }
        }
        c.file = (width - 2.0 * PAD - (cols(&c) as f32 - 1.0) * GAP - fixed(&c)).max(80.0);
        c
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
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(24.0);
        });
        return actions;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .id_salt("downloads-scroll")
        .show(ui, |ui| {
            header(ui);
            for (idx, (record, rate)) in rows.iter().zip(rates.iter()).enumerate() {
                row(
                    ui,
                    idx,
                    record,
                    *rate,
                    selected == Some(record.id),
                    &mut actions,
                );
            }
        });

    actions
}

fn header(ui: &mut Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 24.0), Sense::hover());
    let inner = egui::Rect::from_min_max(
        rect.left_top() + egui::vec2(PAD, 2.0),
        rect.right_bottom() - egui::vec2(PAD, 2.0),
    );
    let mut head = ui.new_child(
        UiBuilder::new()
            .id_salt("header")
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    let cols = Columns::for_width(head.available_width());
    let weak = head.visuals().weak_text_color();
    let text = |ui: &mut Ui, label: &str| {
        ui.label(RichText::new(label).small().strong().color(weak));
    };
    cell(&mut head, cols.state, "h-state", |ui| text(ui, "STATE"));
    cell(&mut head, cols.file, "h-file", |ui| text(ui, "FILE"));
    cell(&mut head, cols.id, "h-id", |ui| text(ui, "ID"));
    cell(&mut head, cols.conns, "h-conns", |ui| text(ui, "CONNECTIONS"));
    if cols.show_added {
        cell(&mut head, cols.added, "h-added", |ui| text(ui, "ADDED"));
    }
    cell(&mut head, cols.progress, "h-progress", |ui| text(ui, "PROGRESS"));
    cell(&mut head, cols.size, "h-size", |ui| text(ui, "SIZE"));
    if cols.show_speed {
        cell(&mut head, cols.speed, "h-speed", |ui| text(ui, "SPEED"));
    }
    if cols.show_eta {
        cell(&mut head, cols.eta, "h-eta", |ui| text(ui, "ETA"));
    }
    cell(&mut head, cols.actions, "h-actions", |ui| text(ui, "ACTIONS"));
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
}

/// Fixed-width, vertically centred cell inside a table row. The full cell
/// rect is reserved up-front so every column stays aligned.
fn cell(ui: &mut Ui, width: f32, salt: &str, content: impl FnOnce(&mut Ui)) {
    let height = ui.available_height();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    let mut child = ui.new_child(
        UiBuilder::new()
            .id_salt(salt)
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    content(&mut child);
}

#[allow(clippy::too_many_arguments)]
fn row(
    ui: &mut Ui,
    idx: usize,
    record: &DownloadRecord,
    rate: f64,
    is_selected: bool,
    actions: &mut Vec<UiAction>,
) {
    let height = 42.0;
    let (rect, response) = ui
        .allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(format!("{}\n{}", record.url, record.output_path));

    // Row background: selection > hover > zebra stripes.
    let visuals = ui.visuals();
    let bg = if is_selected {
        Some(visuals.selection.bg_fill)
    } else if response.hovered() {
        Some(visuals.widgets.hovered.bg_fill)
    } else if idx % 2 == 1 {
        Some(if visuals.dark_mode {
            Color32::from_white_alpha(0.04)
        } else {
            Color32::from_black_alpha(0.03)
        })
    } else {
        None
    };
    if let Some(bg) = bg {
        ui.painter()
            .rect_filled(rect, egui::Rounding::same(4.0), bg);
    }
    if is_selected {
        let accent = egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.left() + 3.0, rect.bottom()),
        );
        ui.painter()
            .rect_filled(accent, egui::Rounding::same(2.0), visuals.selection.stroke.color);
    }

    let inner = egui::Rect::from_min_max(
        rect.left_top() + egui::vec2(PAD + 2.0, 3.0),
        rect.right_bottom() - egui::vec2(PAD, 3.0),
    );
    let mut row_ui = ui.new_child(
        UiBuilder::new()
            .id_salt(("row", record.id))
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    let cols = Columns::for_width(row_ui.available_width());
    let weak = row_ui.visuals().weak_text_color();
    let color = state_color(&row_ui, record.state);

    // STATE
    cell(&mut row_ui, cols.state, "state", |ui| {
        ui.add(
            egui::Label::new(
                RichText::new(format!("{} {}", state_glyph(record.state), record.state))
                    .small()
                    .color(color),
            )
            .truncate(),
        );
    });

    // FILE
    cell(&mut row_ui, cols.file, "file", |ui| {
        ui.add(egui::Label::new(RichText::new(&record.filename).strong()).truncate());
    });

    // ID
    cell(&mut row_ui, cols.id, "id", |ui| {
        ui.add(
            egui::Label::new(
                RichText::new(&record.public_id)
                    .monospace()
                    .small()
                    .color(weak),
            )
            .truncate(),
        );
    });

    // CONNECTIONS
    cell(&mut row_ui, cols.conns, "conns", |ui| {
        ui.label(
            RichText::new(record.max_connections.to_string())
                .monospace()
                .small()
                .color(weak),
        );
    });

    // ADDED
    if cols.show_added {
        let text = util::format_relative(record.created_at);
        cell(&mut row_ui, cols.added, "added", |ui| {
            ui.add(egui::Label::new(RichText::new(text).small().color(weak)).truncate());
        });
    }

    // PROGRESS
    let fraction = progress_of(record);
    cell(&mut row_ui, cols.progress, "progress", |ui| {
        ui.add(
            egui::ProgressBar::new(fraction)
                .desired_width((cols.progress - 6.0).max(40.0))
                .text(format!("{:.1}%", fraction * 100.0)),
        );
    });

    // SIZE
    let total = record
        .total_size
        .map(|s| human::human_bytes(s.max(0) as u64))
        .unwrap_or_else(|| "?".to_string());
    cell(&mut row_ui, cols.size, "size", |ui| {
        ui.add(
            egui::Label::new(
                RichText::new(format!(
                    "{} / {}",
                    human::human_bytes(record.downloaded_size.max(0) as u64),
                    total
                ))
                .monospace()
                .small(),
            )
            .truncate(),
        );
    });

    // SPEED
    if cols.show_speed {
        let text = if rate > 1.0 {
            human::human_rate(rate)
        } else {
            "—".to_string()
        };
        cell(&mut row_ui, cols.speed, "speed", |ui| {
            ui.label(RichText::new(text).monospace().small());
        });
    }

    // ETA
    if cols.show_eta {
        let eta = match (record.total_size, rate) {
            (Some(total), r) if total > 0 && r > 1.0 && record.state.active() => {
                let remaining = (total - record.downloaded_size).max(0) as f64;
                util::format_duration(remaining / r)
            }
            _ => "—".to_string(),
        };
        cell(&mut row_ui, cols.eta, "eta", |ui| {
            ui.label(RichText::new(eta).monospace().small());
        });
    }

    // ACTIONS
    cell(&mut row_ui, cols.actions, "actions", |ui| {
        // Icon buttons: a bit tighter than the app-wide paddings so four of
        // them fit into the column.
        ui.spacing_mut().button_padding = egui::vec2(7.0, 4.0);
        ui.spacing_mut().item_spacing.x = 5.0;
        let running = record.state.active();
        let resumable = !running && record.state != DownloadState::Completed;

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

    // The whole row is one big clickable target.
    if response.clicked() {
        actions.push(UiAction::Select(record.id));
    }
    if response.double_clicked() {
        actions.push(UiAction::OpenDetails(record.id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_panels_keep_every_column() {
        let c = Columns::for_width(1200.0);
        assert!(c.show_added && c.show_speed && c.show_eta);
        assert!(c.file >= 140.0);
    }

    #[test]
    fn narrow_panels_drop_optional_columns_before_the_file_name() {
        let c = Columns::for_width(520.0);
        assert!(!c.show_added || !c.show_speed || !c.show_eta);
        assert!(c.file >= 80.0);
    }

    #[test]
    fn columns_never_exceed_the_available_width() {
        for width in [300.0, 520.0, 830.0, 1100.0, 1600.0] {
            let c = Columns::for_width(width);
            let cols = 7 + (c.show_added as usize) + (c.show_speed as usize) + (c.show_eta as usize);
            let total = 2.0 * PAD + (cols as f32 - 1.0) * GAP + c.file + c.state + c.id + c.conns
                + c.progress
                + c.size
                + c.actions
                + if c.show_added { c.added } else { 0.0 }
                + if c.show_speed { c.speed } else { 0.0 }
                + if c.show_eta { c.eta } else { 0.0 };
            // The row may be a bit narrower than the panel (extra room for the
            // file column), but never wider.
            assert!(total <= width + 0.5, "width {width}: total {total}");
        }
    }
}
