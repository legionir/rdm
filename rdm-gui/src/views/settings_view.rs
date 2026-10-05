//! Settings sidebar: defaults for new downloads + app behaviour.
//! Toggled from the top menu.

use egui::{Align, Layout, Ui};

use crate::settings::AppSettings;
use crate::state::{GuiState, UiAction};
use crate::theme::{self, components, Sizes, Spacing};
use crate::util;

pub fn show(
    ui: &mut Ui,
    settings: &mut AppSettings,
    settings_path: &str,
    db_path: &str,
    state: &mut GuiState,
) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let before = settings.clone();
    let palette = theme::palette_of(ui);
    let sizes = Sizes::default();
    let spacing = Spacing::default();
    let edit_margin = egui::Margin::symmetric(spacing.sm, spacing.xs);

    // Children must not report a bigger size than the panel — TextEdit with
    // infinite desired width and long path labels would otherwise stretch a
    // resizable SidePanel across the whole window (and again on every resize).
    ui.set_max_width(ui.available_width());

    ui.add_space(spacing.xs);
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
    ui.add_space(spacing.xs);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .id_salt("settings-scroll")
        .show(ui, |ui| {
            components::section_title(ui, "Defaults for new downloads");
            ui.add_space(spacing.md);

            ui.label("Download directory");
            ui.horizontal(|ui| {
                let avail = ui.available_width();
                ui.add(
                    egui::TextEdit::singleline(&mut settings.download_dir)
                        .hint_text("current directory")
                        .margin(edit_margin)
                        .desired_width((avail - sizes.picker_reserve).max(sizes.picker_min)),
                );
                if components::icon_button(
                    ui,
                    "📂",
                    "Choose a folder in the system file explorer",
                ) {
                    let start = util::existing_dir(&settings.download_dir);
                    if let Some(dir) = util::pick_folder(start.as_deref(), "Download directory") {
                        settings.download_dir = dir.display().to_string();
                    }
                }
            });

            ui.add_space(spacing.md);
            ui.label("Connections");
            ui.add(egui::Slider::new(&mut settings.connections, 1..=128));

            ui.add_space(spacing.xs);
            ui.label("Retries per chunk");
            ui.add(egui::DragValue::new(&mut settings.retries).range(0..=100));

            ui.add_space(spacing.md);
            ui.label("Minimum chunk size");
            ui.add(
                egui::TextEdit::singleline(&mut settings.chunk_size)
                    .hint_text("1MiB")
                    .margin(edit_margin)
                    .desired_width(ui.available_width()),
            );

            ui.add_space(spacing.xs);
            ui.label("Speed limit");
            ui.add(
                egui::TextEdit::singleline(&mut settings.max_speed)
                    .hint_text("unlimited, e.g. 5MB/s")
                    .margin(edit_margin)
                    .desired_width(ui.available_width()),
            );

            ui.add_space(spacing.xs);
            ui.label("Connect timeout (s)");
            ui.add(egui::DragValue::new(&mut settings.timeout_secs).range(1..=3600));

            ui.add_space(spacing.xs);
            ui.label("User agent");
            ui.add(
                egui::TextEdit::singleline(&mut settings.user_agent)
                    .hint_text("rdm/0.1.0")
                    .margin(edit_margin)
                    .desired_width(ui.available_width()),
            );

            ui.add_space(spacing.xl);
            ui.separator();
            components::section_title(ui, "Application");
            ui.add_space(spacing.md);

            ui.label("Metadata directory (--data-dir)");
            ui.horizontal(|ui| {
                let avail = ui.available_width();
                ui.add(
                    egui::TextEdit::singleline(&mut state.data_dir_input)
                        .hint_text(".rdm")
                        .margin(edit_margin)
                        .desired_width((avail - sizes.picker_reserve).max(sizes.picker_min)),
                );
                if components::icon_button(
                    ui,
                    "📂",
                    "Choose a folder in the system file explorer",
                ) {
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

            ui.add_space(spacing.md);
            ui.label("Max concurrent downloads")
                .on_hover_text("0 = unlimited; extra downloads wait in the queue");
            ui.add(egui::Slider::new(&mut settings.max_concurrent, 0..=16));

            ui.add_space(spacing.xs);
            ui.label("Table refresh (ms)");
            ui.add(egui::Slider::new(&mut settings.refresh_ms, 100..=5000));

            ui.add_space(spacing.xs);
            ui.checkbox(&mut settings.confirm_remove, "Confirm destructive actions")
                .on_hover_text(
                    "Ask before removing downloads or restarting one from scratch; \
                     the dialog always names the consequence",
                );
            ui.checkbox(
                &mut settings.purge_on_remove,
                "Delete files too when removing",
            );
            ui.checkbox(&mut settings.dark_mode, "Dark theme");

            ui.add_space(spacing.sm);
            ui.separator();
            ui.label("Desktop integration").strong();
            ui.checkbox(
                &mut settings.clipboard_prefill,
                "Fill the URL from the clipboard",
            )
            .on_hover_text(
                "When “New download” opens, a link on the clipboard is inserted \
                 and selected, so Start (or Enter) begins it immediately",
            );
            ui.checkbox(&mut settings.close_to_tray, "Keep running in the tray")
                .on_hover_text(
                    "Closing the window hides it instead of quitting; the transfer \
                     keeps running and the tray menu brings it back. The tray's \
                     “Quit rdm” really exits",
                );
            ui.checkbox(
                &mut settings.drop_target_enabled,
                "Floating drop target",
            )
            .on_hover_text(
                "A small always-on-top box above the taskbar clock: drop a link \
                 on it (from a browser) and the New download form opens with it",
            );

            ui.add_space(spacing.md);
            ui.label("Engine log verbosity")
                .on_hover_text("Same levels as the CLI's -v / -vv / -vvv; RUST_LOG still wins");
            egui::ComboBox::from_id_salt("log-level")
                .selected_text(settings.log_level.clone())
                .show_ui(ui, |ui| {
                    for level in crate::logging::LEVELS {
                        ui.selectable_value(&mut settings.log_level, level.to_string(), level);
                    }
                });

            ui.add_space(spacing.xl);
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
                components::banner(
                    ui,
                    &palette,
                    components::Level::Warning,
                    "unsaved changes",
                );
            }
            ui.add_space(spacing.md);
            components::hint(ui, &palette, settings_path);
            components::hint(ui, &palette, db_path);
        });

    if *settings != before {
        state.settings_dirty = true;
    }
    actions
}
