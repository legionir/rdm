//! The download table (`rdm list` with buttons).
//!
//! Rows are full-width: clicking anywhere on a row selects it and
//! double-clicking opens the details modal. Every colour, size and spacing
//! comes from the design tokens — the responsive column arithmetic is the only
//! layout logic that lives here, and it is unit-tested.

use egui::{Align, Layout, RichText, Sense, Ui, UiBuilder};
use rdm::models::{DownloadRecord, DownloadState};
use rdm::utils::human;

use crate::state::{progress_of, GuiState, UiAction};
use crate::theme::{self, components, Palette, Radii, Sizes, Spacing, Table};
use crate::util;
use crate::ux;

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
    /// Fit the columns into `width`.
    ///
    /// Drop order for the optional columns is ADDED, SPEED, ETA; then the
    /// progress bar gets narrow, and finally the fixed columns are scaled down
    /// proportionally (never below a quarter of their design width) so the row
    /// never grows wider than the panel. FILE keeps at least `file_min`.
    fn for_tokens(width: f32, t: &Table) -> Self {
        let mut c = Columns {
            state: t.state,
            file: t.file_ideal,
            id: t.id,
            conns: t.conns,
            added: t.added,
            progress: t.progress,
            size: t.size,
            speed: t.speed,
            eta: t.eta,
            actions: t.actions,
            show_added: true,
            show_speed: true,
            show_eta: true,
        };
        let fixed = |c: &Columns| {
            c.state
                + c.id
                + c.conns
                + c.progress
                + c.size
                + c.actions
                + if c.show_added { c.added } else { 0.0 }
                + if c.show_speed { c.speed } else { 0.0 }
                + if c.show_eta { c.eta } else { 0.0 }
        };
        // 7 columns are always shown (state, file, id, conns, progress, size,
        // actions); the three optional ones can be dropped on narrow panels.
        let cols = |c: &Columns| {
            7 + (c.show_added as usize) + (c.show_speed as usize) + (c.show_eta as usize)
        };
        let budget =
            |c: &Columns| width - 2.0 * t.pad - (cols(c) as f32 - 1.0) * t.gap - t.file_ideal;
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
            c.progress = t.progress_compact;
        }
        // Still too narrow? Scale the fixed columns down proportionally so the
        // row never grows wider than the panel (FILE keeps its minimum).
        let avail_fixed = width - 2.0 * t.pad - (cols(&c) as f32 - 1.0) * t.gap - t.file_min;
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
        c.file = (width - 2.0 * t.pad - (cols(&c) as f32 - 1.0) * t.gap - fixed(&c)).max(t.file_min);
        c
    }
}

/// Columns are laid out back-to-front so a resize never overflows the panel.
/// Used by the unit tests; the runtime code relies on `for_tokens` itself.
#[cfg(test)]
fn columns_width(c: &Columns, t: &Table) -> f32 {
    2.0 * t.pad
        + 6.0 * t.gap
        + c.file
        + c.state
        + c.id
        + c.conns
        + c.progress
        + c.size
        + c.actions
        + if c.show_added { c.added + t.gap } else { 0.0 }
        + if c.show_speed { c.speed + t.gap } else { 0.0 }
        + if c.show_eta { c.eta + t.gap } else { 0.0 }
}

pub fn show(ui: &mut Ui, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let palette = theme::palette_of(ui);
    let selected = state.selected;
    let rows: Vec<DownloadRecord> = state.visible_rows().into_iter().cloned().collect();
    let rates: Vec<f64> = rows.iter().map(|r| state.rate_of(r.id)).collect();

    if rows.is_empty() {
        let spacing = Spacing::default();
        ui.vertical_centered(|ui| {
            ui.add_space(spacing.xxl);
            components::hint(
                ui,
                &palette,
                "Nothing here yet — press “New download” and paste a URL to start your first \
                 download. Press F1 for what the states mean.",
            );
            ui.add_space(spacing.xxl);
        });
        return actions;
    }

    // One geometry per frame: the header and every row share it, so a resize
    // can never make a row drop a column the header still shows.
    let table = Sizes::default().table;
    let columns = Columns::for_tokens(ui.available_width() - 2.0 * table.pad, &table);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .id_salt("downloads-scroll")
        .show(ui, |ui| {
            header(ui, &palette, &columns);
            for (idx, (record, rate)) in rows.iter().zip(rates.iter()).enumerate() {
                row(
                    ui,
                    idx,
                    record,
                    *rate,
                    selected == Some(record.id),
                    &palette,
                    &columns,
                    &mut actions,
                );
            }
        });

    actions
}

fn header(ui: &mut Ui, palette: &Palette, cols: &Columns) {
    let table = Sizes::default().table;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width, table.header_height),
        Sense::hover(),
    );
    let inner = egui::Rect::from_min_max(
        rect.left_top() + egui::vec2(table.pad, table.header_inset),
        rect.right_bottom() - egui::vec2(table.pad, table.header_inset),
    );
    let mut head = ui.new_child(
        UiBuilder::new()
            .id_salt("header")
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    let text = |ui: &mut Ui, label: &str| {
        ui.label(RichText::new(label).small().strong().color(palette.text_muted));
    };
    cell(&mut head, cols.state, "h-state", |ui| text(ui, "STATE"));
    cell(&mut head, cols.file, "h-file", |ui| text(ui, "FILE"));
    cell(&mut head, cols.id, "h-id", |ui| text(ui, "ID"));
    cell(&mut head, cols.conns, "h-conns", |ui| text(ui, "CONNECTIONS"));
    if cols.show_added {
        cell(&mut head, cols.added, "h-added", |ui| text(ui, "ADDED"));
    }
    cell(&mut head, cols.progress, "h-progress", |ui| {
        text(ui, "PROGRESS")
    });
    cell(&mut head, cols.size, "h-size", |ui| text(ui, "SIZE"));
    if cols.show_speed {
        cell(&mut head, cols.speed, "h-speed", |ui| text(ui, "SPEED"));
    }
    if cols.show_eta {
        cell(&mut head, cols.eta, "h-eta", |ui| text(ui, "ETA"));
    }
    cell(&mut head, cols.actions, "h-actions", |ui| {
        text(ui, "ACTIONS")
    });
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        egui::Stroke::new(1.0, palette.border_subtle),
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
    palette: &Palette,
    cols: &Columns,
    actions: &mut Vec<UiAction>,
) {
    let spacing = Spacing::default();
    let sizes = Sizes::default();
    let table = sizes.table;
    let radii = Radii::default();

    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), spacing.row_height),
        Sense::click(),
    );
    let response = response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(format!("{}\n{}", record.url, record.output_path));

    // Row background: selected > hovered > zebra stripes (token precedence).
    if let Some(bg) = components::row_background(palette, is_selected, response.hovered(), idx % 2 == 1)
    {
        ui.painter().rect_filled(rect, egui::Rounding::same(radii.md), bg);
    }
    // The accent bar is the non-colour-only signal for "selected" (SC 1.4.11).
    if is_selected {
        let accent = egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.left() + table.accent, rect.bottom()),
        );
        ui.painter()
            .rect_filled(accent, egui::Rounding::same(radii.sm), palette.accent);
    }

    let inner = egui::Rect::from_min_max(
        rect.left_top() + egui::vec2(table.pad, table.row_inset),
        rect.right_bottom() - egui::vec2(table.pad, table.row_inset),
    );
    let mut row_ui = ui.new_child(
        UiBuilder::new()
            .id_salt(("row", record.id))
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );

    // STATE — glyph + label + semantic colour on a chip surface; the tooltip
    // carries the legend definition and the next step for that state.
    cell(&mut row_ui, cols.state, "state", |ui| {
        components::status_chip(ui, palette, record.state)
            .on_hover_text(ux::legend_for(record.state).hover_text());
    });

    // FILE
    cell(&mut row_ui, cols.file, "file", |ui| {
        ui.add(
            egui::Label::new(
                RichText::new(&record.filename)
                    .strong()
                    .color(palette.text_primary),
            )
            .truncate(),
        );
    });

    // ID
    cell(&mut row_ui, cols.id, "id", |ui| {
        components::meta(ui, palette, record.public_id.as_str());
    });

    // CONNECTIONS
    cell(&mut row_ui, cols.conns, "conns", |ui| {
        components::meta(ui, palette, record.max_connections.to_string());
    });

    // ADDED
    if cols.show_added {
        let text = util::format_relative(record.created_at);
        cell(&mut row_ui, cols.added, "added", |ui| {
            components::meta(ui, palette, text);
        });
    }

    // PROGRESS
    let fraction = progress_of(record);
    cell(&mut row_ui, cols.progress, "progress", |ui| {
        ui.add(
            egui::ProgressBar::new(fraction)
                .desired_width((cols.progress - sizes.progress_inset).max(sizes.progress_min))
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
                .small()
                .color(palette.text_primary),
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
            components::meta(ui, palette, text);
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
            components::meta(ui, palette, eta);
        });
    }

    // ACTIONS
    cell(&mut row_ui, cols.actions, "actions", |ui| {
        // Square icon buttons (the shared control height) with a tight gap, so
        // up to six of them fit into the actions column.
        ui.spacing_mut().item_spacing.x = spacing.icon_gap;
        let running = record.state.active();
        // Resume only where the backend accepts it: `Backend::resume` rejects
        // terminal states except Cancelled, so a Failed download is offered
        // Restart instead of a button that could only produce an error.
        let resumable = !running
            && !matches!(
                record.state,
                DownloadState::Completed | DownloadState::Failed
            );

        if running {
            if components::icon_button(ui, crate::theme::Icon::Pause, ux::pause_tooltip()).clicked() {
                actions.push(UiAction::Pause(record.id));
            }
            if components::icon_button(ui, crate::theme::Icon::Stop, ux::cancel_tooltip()).clicked() {
                actions.push(UiAction::Cancel(record.id));
            }
        } else if resumable && components::icon_button(ui, crate::theme::Icon::Play, ux::resume_tooltip()).clicked() {
            actions.push(UiAction::Resume(record.id));
        }
        if !running
            && components::icon_button(ui, crate::theme::Icon::Restart, ux::restart_tooltip(record.state)).clicked() {
            actions.push(UiAction::AskRestart(record.id));
        }
        if components::icon_button(ui, crate::theme::Icon::Folder, ux::open_folder_tooltip()).clicked() {
            actions.push(UiAction::OpenOutputFolder(record.id));
        }
        if running {
            // Disabled with an explanation instead of failing in the backend.
            let disabled = ui.add_enabled(
                false,
                egui::Button::new("").min_size(egui::Vec2::splat(
                    crate::theme::Spacing::default().icon_button_side,
                )),
            );
            disabled.on_disabled_hover_text(ux::remove_tooltip(true));
        } else if components::icon_button(ui, crate::theme::Icon::Trash, ux::remove_tooltip(false)).clicked() {
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

    fn for_width(width: f32) -> Columns {
        Columns::for_tokens(width, &Sizes::default().table)
    }

    #[test]
    fn wide_panels_keep_every_column() {
        let c = for_width(1200.0);
        assert!(c.show_added && c.show_speed && c.show_eta);
        assert!(c.file >= 140.0);
    }

    #[test]
    fn narrow_panels_drop_optional_columns_before_the_file_name() {
        let c = for_width(520.0);
        assert!(!c.show_added || !c.show_speed || !c.show_eta);
        assert!(c.file >= 80.0);
    }

    #[test]
    fn columns_never_exceed_the_available_width() {
        let table = Sizes::default().table;
        for width in [300.0, 520.0, 830.0, 1100.0, 1600.0] {
            let c = for_width(width);
            let total = columns_width(&c, &table);
            // The row may be a bit narrower than the panel (extra room for the
            // file column), but never wider.
            assert!(total <= width + 0.5, "width {width}: total {total}");
        }
    }

    #[test]
    fn optional_columns_are_dropped_in_priority_order() {
        let table = Sizes::default().table;
        let wide = Columns::for_tokens(1600.0, &table);
        assert!(wide.show_added && wide.show_speed && wide.show_eta);
        let medium = Columns::for_tokens(900.0, &table);
        assert!(!medium.show_added || !medium.show_speed, "ADDED goes first");
        let narrow = Columns::for_tokens(560.0, &table);
        assert!(!narrow.show_eta);
    }
}
