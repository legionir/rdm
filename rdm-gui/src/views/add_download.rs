//! "New download" window — every flag of `rdm download` as a widget.

use egui::{Context, RichText};

use crate::state::{GuiState, UiAction};

pub fn show(ctx: &Context, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();
    if !state.show_add {
        return actions;
    }
    let mut open = true;
    let mut submit = false;
    let mut cancel = false;

    egui::Window::new("New download")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            egui::Grid::new("add-form")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .min_col_width(120.0)
                .show(ui, |ui| {
                    ui.label("URL");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.form.url)
                            .hint_text("https://example.com/file.zip")
                            .desired_width(380.0),
                    );
                    ui.end_row();

                    ui.label("Output")
                        .on_hover_text("File path or directory. Empty = current directory.");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.form.output)
                            .hint_text("directory or full file path")
                            .desired_width(380.0),
                    );
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
                            .desired_width(160.0),
                    );
                    ui.end_row();

                    ui.label("Speed limit")
                        .on_hover_text("--max-speed, e.g. 5MB/s. Empty = unlimited.");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.form.max_speed)
                            .hint_text("unlimited")
                            .desired_width(160.0),
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
                            .desired_width(380.0),
                    );
                    ui.end_row();

                    ui.label("User agent").on_hover_text("--user-agent");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.form.user_agent)
                            .hint_text("rdm/0.1.0")
                            .desired_width(380.0),
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
                ui.add_space(4.0);
                ui.colored_label(egui::Color32::from_rgb(239, 68, 68), err);
            }

            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Start").strong()).clicked() {
                    submit = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
                ui.add_space(12.0);
                ui.small("Tip: a directory in “Output” keeps the server-provided filename.");
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
