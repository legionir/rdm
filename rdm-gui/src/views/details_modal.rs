//! Details modal — opened by double-clicking a row (or with `Enter`). Hosts
//! the tabs that used to live in the bottom panel: Overview, Chunks and JSON
//! (`rdm info`).
//!
//! All colours, sizes and spacings come from the design tokens; the chunk
//! statuses share the same semantic roles as the download states, so a
//! "Completed" chunk and a "Completed" download are the same green.

use egui::{Context, RichText, Ui};
use rdm::models::{ChunkRecord, DownloadRecord};
use rdm::utils::human;

use crate::state::{DetailTab, GuiState, UiAction};
use crate::theme::{self, components, Palette, Sizes, Spacing};
use crate::util;

pub fn show(ctx: &Context, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let Some(id) = state.detail_id else {
        return actions;
    };
    let Some(record) = state.record(id).cloned() else {
        // The record was removed while the modal was open.
        state.detail_id = None;
        return actions;
    };

    let palette = theme::palette_ctx(ctx);
    let sizes = Sizes::default();
    let spacing = Spacing::default();
    let mut open = true;
    egui::Window::new(format!("Details — {}", record.filename))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(sizes.details_default)
        .min_size(sizes.details_min)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(spacing.xxs);
            ui.horizontal_wrapped(|ui| {
                for tab in DetailTab::ALL {
                    if ui
                        .selectable_label(state.detail_tab == tab, tab.title())
                        .clicked()
                    {
                        state.detail_tab = tab;
                    }
                }
                ui.separator();
                ui.label(components::state_text(&palette, record.state));
                components::hint(ui, &palette, format!("· {}", record.filename));
            });
            ui.separator();
            ui.add_space(spacing.sm);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .id_salt("details-modal-scroll")
                .show(ui, |ui| match state.detail_tab {
                    DetailTab::Overview => overview(ui, &record, state, &palette, &mut actions),
                    DetailTab::Chunks => chunks(ui, state, &palette),
                    DetailTab::Json => json(ui, state, &mut actions),
                });
        });

    if !open {
        state.detail_id = None;
    }
    actions
}

fn overview(
    ui: &mut Ui,
    record: &DownloadRecord,
    state: &GuiState,
    palette: &Palette,
    actions: &mut Vec<UiAction>,
) {
    let sizes = Sizes::default();
    egui::Grid::new("detail-overview")
        .num_columns(2)
        .spacing(sizes.details_grid_spacing)
        .min_col_width(sizes.details_grid_key_width)
        .striped(true)
        .show(ui, |ui| {
            let field = |ui: &mut Ui, key: &str, value: String| {
                components::key_label(ui, palette, key);
                ui.label(
                    RichText::new(value)
                        .monospace()
                        .small()
                        .color(palette.text_primary),
                );
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
                components::key_label(ui, palette, "last error");
                ui.add(
                    egui::Label::new(
                        RichText::new(err)
                            .monospace()
                            .small()
                            .color(palette.danger),
                    )
                    .wrap(),
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

    ui.add_space(sizes.details_grid_spacing[1]);
    ui.horizontal_wrapped(|ui| {
        if ui.button("Copy URL").clicked() {
            actions.push(UiAction::CopyToClipboard(record.url.clone()));
        }
        if ui.button("Copy output path").clicked() {
            actions.push(UiAction::CopyToClipboard(record.output_path.clone()));
        }
        if ui.button("Open folder").clicked() {
            actions.push(UiAction::OpenOutputFolder(record.id));
        }
        if ui.button("Copy CLI command").clicked() {
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

fn chunks(ui: &mut Ui, state: &GuiState, palette: &Palette) {
    let sizes = Sizes::default();
    if state.chunks.is_empty() {
        components::hint(ui, palette, "No chunk rows yet.");
        return;
    }
    egui::Grid::new("detail-chunks")
        .num_columns(7)
        .striped(true)
        .spacing(sizes.chunk_grid_spacing)
        .show(ui, |ui| {
            for head in ["#", "START", "END", "DONE", "PROGRESS", "STATUS", "ERROR"] {
                ui.label(
                    RichText::new(head)
                        .small()
                        .strong()
                        .color(palette.text_muted),
                );
            }
            ui.end_row();
            for chunk in &state.chunks {
                chunk_row(ui, chunk, palette, &sizes);
            }
        });
}

fn chunk_row(ui: &mut Ui, chunk: &ChunkRecord, palette: &Palette, sizes: &Sizes) {
    let number = |ui: &mut Ui, value: String| {
        ui.label(
            RichText::new(value)
                .monospace()
                .small()
                .color(palette.text_primary),
        );
    };
    number(ui, chunk.idx.to_string());
    number(ui, chunk.start.to_string());
    number(ui, chunk.end.to_string());
    ui.label(
        RichText::new(human::human_bytes(chunk.downloaded.max(0) as u64))
            .monospace()
            .small()
            .color(palette.text_primary),
    );
    let len = chunk.len().max(1) as f32;
    ui.add(
        egui::ProgressBar::new((chunk.downloaded.max(0) as f32 / len).clamp(0.0, 1.0))
            .desired_width(sizes.progress_chunk),
    );
    ui.label(
        RichText::new(chunk.status.to_string())
            .small()
            .color(palette.chunk_color(chunk.status)),
    );
    ui.label(
        RichText::new(chunk.error.clone().unwrap_or_default())
            .small()
            .color(palette.danger),
    );
    ui.end_row();
}

fn json(ui: &mut Ui, state: &GuiState, actions: &mut Vec<UiAction>) {
    let spacing = Spacing::default();
    if ui.button("Copy JSON").clicked() {
        actions.push(UiAction::CopyToClipboard(state.json.clone()));
    }
    ui.add_space(spacing.sm);
    let mut text = state.json.clone();
    ui.add(
        egui::TextEdit::multiline(&mut text)
            .code_editor()
            .desired_width(f32::INFINITY)
            .desired_rows(Sizes::default().json_rows)
            .interactive(false),
    );
}
