//! Settings sidebar: defaults for new downloads + app behaviour.
//! Toggled from the top menu.

use egui::{Align, Color32, Layout, RichText, Ui};

use crate::settings::AppSettings;
use crate::state::{GuiState, UiAction};
use crate::util;

const EDIT_MARGIN: egui::Margin = egui::Margin::symmetric(6.0, 4.0);

pub fn show(
    ui: &mut Ui,
    settings: &mut AppSettings,
    settings_path: &str,
    db_path: &str,
    state: &mut GuiState,
) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let before = settings.clone();

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.heading("Settings");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .button("✕")
                .on_hover_text("Hide the settings sidebar")
                .clicked()
            {
                state.show_settings = false;
            }
        });
    });
    ui.separator();
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .id_salt("settings-scroll")
        .show(ui, |ui| {
            ui.label(RichText::new("Defaults for new downloads").strong());
            ui.add_space(8.0);

            ui.label("Download directory");
            ui.horizontal(|ui| {
                let avail = ui.available_width();
                ui.add(
                    egui::TextEdit::singleline(&mut settings.download_dir)
                        .hint_text("current directory")
                        .margin(EDIT_MARGIN)
                        .desired_width((avail - 52.0).max(120.0)),
                );
                if ui
                    .button("📂")
                    .on_hover_text("Choose a folder in the system file explorer")
                    .clicked()
                {
                    let start = util::existing_dir(&settings.download_dir);
                    if let Some(dir) = util::pick_folder(start.as_deref(), "Download directory") {
                        settings.download_dir = dir.display().to_string();
                    }
                }
            });

            ui.add_space(8.0);
            ui.label("Connections");
            ui.add(egui::Slider::new(&mut settings.connections, 1..=128));

            ui.add_space(4.0);
            ui.label("Retries per chunk");
            ui.add(egui::DragValue::new(&mut settings.retries).range(0..=100));

            ui.add_space(8.0);
            ui.label("Minimum chunk size");
            ui.add(
                egui::TextEdit::singleline(&mut settings.chunk_size)
                    .hint_text("1MiB")
                    .margin(EDIT_MARGIN)
                    .desired_width(f32::INFINITY),
            );

            ui.add_space(4.0);
            ui.label("Speed limit");
            ui.add(
                egui::TextEdit::singleline(&mut settings.max_speed)
                    .hint_text("unlimited, e.g. 5MB/s")
                    .margin(EDIT_MARGIN)
                    .desired_width(f32::INFINITY),
            );

            ui.add_space(4.0);
            ui.label("Connect timeout (s)");
            ui.add(egui::DragValue::new(&mut settings.timeout_secs).range(1..=3600));

            ui.add_space(4.0);
            ui.label("User agent");
            ui.add(
                egui::TextEdit::singleline(&mut settings.user_agent)
                    .hint_text("rdm/0.1.0")
                    .margin(EDIT_MARGIN)
                    .desired_width(f32::INFINITY),
            );

            ui.add_space(12.0);
            ui.separator();
            ui.label(RichText::new("Application").strong());
            ui.add_space(8.0);

            ui.label("Metadata directory (--data-dir)");
            ui.horizontal(|ui| {
                let avail = ui.available_width();
                ui.add(
                    egui::TextEdit::singleline(&mut state.data_dir_input)
                        .hint_text(".rdm")
                        .margin(EDIT_MARGIN)
                        .desired_width((avail - 52.0).max(120.0)),
                );
                if ui
                    .button("📂")
                    .on_hover_text("Choose a folder in the system file explorer")
                    .clicked()
                {
                    let start = util::existing_dir(&state.data_dir_input);
                    if let Some(dir) = util::pick_folder(start.as_deref(), "Metadata directory") {
                        state.data_dir_input = dir.display().to_string();
                    }
                }
            });
            if ui
                .button("Apply data directory")
                .on_hover_text("Reopens metadata.db from another folder")
                .clicked()
            {
                actions.push(UiAction::ApplyDataDir);
            }

            ui.add_space(8.0);
            ui.label("Max concurrent downloads")
                .on_hover_text("0 = unlimited; extra jobs wait in the queue");
            ui.add(egui::Slider::new(&mut settings.max_concurrent, 0..=16));

            ui.add_space(4.0);
            ui.label("Table refresh (ms)");
            ui.add(egui::Slider::new(&mut settings.refresh_ms, 100..=5000));

            ui.add_space(4.0);
            ui.checkbox(&mut settings.confirm_remove, "Confirm before removing");
            ui.checkbox(
                &mut settings.purge_on_remove,
                "Delete files too when removing",
            );
            ui.checkbox(&mut settings.dark_mode, "Dark theme");

            ui.add_space(8.0);
            ui.label("Engine log verbosity")
                .on_hover_text("Same levels as the CLI's -v / -vv / -vvv; RUST_LOG still wins");
            egui::ComboBox::from_id_salt("log-level")
                .selected_text(settings.log_level.clone())
                .show_ui(ui, |ui| {
                    for level in crate::logging::LEVELS {
                        ui.selectable_value(&mut settings.log_level, level.to_string(), level);
                    }
                });

            ui.add_space(12.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("💾 Save").clicked() {
                    actions.push(UiAction::SaveSettings);
                }
                if ui.button("↺ Reload").clicked() {
                    actions.push(UiAction::ReloadSettings);
                }
            });
            if state.settings_dirty {
                ui.label(
                    RichText::new("unsaved changes")
                        .small()
                        .color(Color32::from_rgb(217, 119, 6)),
                );
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new(settings_path)
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
            ui.label(
                RichText::new(db_path)
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        });

    if *settings != before {
        state.settings_dirty = true;
    }
    actions
}
