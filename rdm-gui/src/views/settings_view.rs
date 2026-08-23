//! Settings side panel: defaults for new downloads + app behaviour.

use egui::{Color32, RichText, Ui};

use crate::settings::AppSettings;
use crate::state::{GuiState, UiAction};

pub fn show(
    ui: &mut Ui,
    settings: &mut AppSettings,
    settings_path: &str,
    state: &mut GuiState,
) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let before = settings.clone();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .id_salt("settings-scroll")
        .show(ui, |ui| {
            ui.label(RichText::new("Defaults for new downloads").strong());
            ui.add_space(4.0);

            ui.label("Download directory");
            ui.add(
                egui::TextEdit::singleline(&mut settings.download_dir)
                    .hint_text("current directory")
                    .desired_width(f32::INFINITY),
            );

            ui.add_space(6.0);
            ui.label("Connections");
            ui.add(egui::Slider::new(&mut settings.connections, 1..=128));

            ui.label("Retries per chunk");
            ui.add(egui::DragValue::new(&mut settings.retries).range(0..=100));

            ui.add_space(6.0);
            ui.label("Minimum chunk size");
            ui.add(
                egui::TextEdit::singleline(&mut settings.chunk_size)
                    .hint_text("1MiB")
                    .desired_width(f32::INFINITY),
            );

            ui.label("Speed limit");
            ui.add(
                egui::TextEdit::singleline(&mut settings.max_speed)
                    .hint_text("unlimited, e.g. 5MB/s")
                    .desired_width(f32::INFINITY),
            );

            ui.label("Connect timeout (s)");
            ui.add(egui::DragValue::new(&mut settings.timeout_secs).range(1..=3600));

            ui.label("User agent");
            ui.add(
                egui::TextEdit::singleline(&mut settings.user_agent)
                    .hint_text("rdm/0.1.0")
                    .desired_width(f32::INFINITY),
            );

            ui.add_space(10.0);
            ui.separator();
            ui.label(RichText::new("Application").strong());
            ui.add_space(4.0);

            ui.label("Metadata directory (--data-dir)");
            ui.add(
                egui::TextEdit::singleline(&mut state.data_dir_input)
                    .hint_text(".rdm")
                    .desired_width(f32::INFINITY),
            );
            if ui
                .button("Apply data directory")
                .on_hover_text("Reopens metadata.db from another folder")
                .clicked()
            {
                actions.push(UiAction::ApplyDataDir);
            }

            ui.add_space(6.0);
            ui.label("Table refresh (ms)");
            ui.add(egui::Slider::new(&mut settings.refresh_ms, 100..=5000));
            ui.checkbox(&mut settings.confirm_remove, "Confirm before removing");
            ui.checkbox(
                &mut settings.purge_on_remove,
                "Delete files too when removing",
            );
            ui.checkbox(&mut settings.dark_mode, "Dark theme");

            ui.add_space(10.0);
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
                        .color(Color32::from_rgb(245, 158, 11)),
                );
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new(settings_path)
                    .small()
                    .color(Color32::from_gray(130)),
            );
        });

    if *settings != before {
        state.settings_dirty = true;
    }
    actions
}
