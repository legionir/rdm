//! "New download" window — every flag of `rdm download` as a widget.

use egui::Context;

use crate::state::{GuiState, UiAction};
use crate::theme::{self, components, Level, Sizes, Spacing};
use crate::util;
use crate::ux;

pub fn show(ctx: &Context, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();
    if !state.show_add {
        return actions;
    }
    let mut open = true;
    let mut submit = false;
    let mut cancel = false;

    let palette = theme::palette_ctx(ctx);
    let sizes = Sizes::default();
    let spacing = Spacing::default();
    let edit_margin = egui::Margin::symmetric(spacing.sm, spacing.xs);
    // Set when the window was opened from the tray/drop target or pre-filled
    // from the clipboard: put the caret in the URL field with the link
    // selected, so typing replaces it and Enter starts the download.
    let focus_url = std::mem::take(&mut state.focus_url);
    let url_field = egui::Id::new("rdm-new-url");

    egui::Window::new("New download")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(sizes.form_width)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            egui::Grid::new("add-form")
                .num_columns(2)
                .spacing(sizes.form_spacing)
                .min_col_width(sizes.form_label_width)
                .show(ui, |ui| {
                    ui.label("URL").on_hover_text(ux::URL_FIELD_HINT);
                    let url = ui.add(
                        egui::TextEdit::singleline(&mut state.form.url)
                            .id(url_field)
                            .hint_text("https://example.com/file.zip")
                            .margin(edit_margin)
                            .desired_width(sizes.edit_wide),
                    );
                    if focus_url {
                        url.request_focus();
                        // Select the whole link: the next keystroke replaces it.
                        if let Some(mut edit) = egui::text_edit::TextEditState::load(ctx, url_field)
                        {
                            let end = state.form.url.chars().count();
                            edit.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                                egui::text::CCursor::new(0),
                                egui::text::CCursor::new(end),
                            )));
                            edit.store(ctx, url_field);
                        }
                    }
                    ui.end_row();

                    ui.label("Output")
                        .on_hover_text("File path or directory. Empty = current directory.");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut state.form.output)
                                .hint_text("directory or full file path")
                                .margin(edit_margin)
                                .desired_width(sizes.edit_medium),
                        );
                        if ui
                            .button("📂")
                            .on_hover_text("Choose a folder in the system file explorer")
                            .clicked()
                        {
                            let start = util::existing_dir(&state.form.output);
                            if let Some(dir) = util::pick_folder(start.as_deref(), "Output folder") {
                                state.form.output = dir.display().to_string();
                            }
                        }
                    });
                    ui.end_row();

                    ui.label("Connections")
                        .on_hover_text("--connections (1..=128)");
                    ui.add(egui::Slider::new(&mut state.form.connections, 1..=128));
                    ui.end_row();

                    ui.label("Retries per chunk").on_hover_text("--retry");
                    ui.add(egui::DragValue::new(&mut state.form.retries).range(0..=100));
                    ui.end_row();

                    ui.label("Min chunk size").on_hover_text("--chunk-size, e.g. 1MiB");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.form.chunk_size)
                            .hint_text("1MiB")
                            .margin(edit_margin)
                            .desired_width(sizes.edit_narrow),
                    );
                    ui.end_row();

                    ui.label("Speed limit")
                        .on_hover_text("--max-speed, e.g. 5MB/s. Empty = unlimited.");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.form.max_speed)
                            .hint_text("unlimited")
                            .margin(edit_margin)
                            .desired_width(sizes.edit_narrow),
                    );
                    ui.end_row();

                    ui.label("Connect timeout").on_hover_text("--timeout (seconds)");
                    ui.add(egui::DragValue::new(&mut state.form.timeout_secs).range(1..=3600));
                    ui.end_row();

                    ui.label("Checksum")
                        .on_hover_text("--checksum sha256:<64 hex chars>");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.form.checksum)
                            .hint_text("sha256:…")
                            .margin(edit_margin)
                            .desired_width(sizes.edit_wide),
                    );
                    ui.end_row();

                    ui.label("User agent").on_hover_text("--user-agent");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.form.user_agent)
                            .hint_text("rdm/0.1.0")
                            .margin(edit_margin)
                            .desired_width(sizes.edit_wide),
                    );
                    ui.end_row();

                    ui.label("Existing record");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut state.form.resume, "resume")
                            .on_hover_text("--resume: continue an incomplete download");
                        ui.checkbox(&mut state.form.force, "force")
                            .on_hover_text("--force: wipe chunks and start over");
                    });
                    ui.end_row();
                });

            if let Some(err) = &state.form_error {
                ui.add_space(spacing.sm);
                components::banner(ui, &palette, Level::Error, err);
            }

            ui.add_space(spacing.lg);
            ui.separator();
            ui.horizontal(|ui| {
                if components::primary_button(ui, "Start")
                    .on_hover_text("Add the download; it starts as soon as a slot is free")
                    .clicked()
                {
                    submit = true;
                }
                if ui
                    .button("Cancel")
                    .on_hover_text("Close without adding anything")
                    .clicked()
                {
                    cancel = true;
                }
                ui.add_space(spacing.xl);
                components::hint(ui, &palette, ux::ADD_TIP);
            });
        });

    if submit {
        actions.push(UiAction::SubmitNewDownload);
    }
    if cancel || !open {
        state.show_add = false;
        state.form_error = None;
    }
    actions
}
