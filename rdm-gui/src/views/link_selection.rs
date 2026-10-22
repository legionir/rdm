//! Checkbox window for the browser's “Download all links with RDM” command.

use egui::Context;

use crate::state::{GuiState, UiAction};
use crate::theme::{self, components, Spacing};

pub fn show(ctx: &Context, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let Some(selection) = state.link_selection.as_mut() else {
        return actions;
    };
    let palette = theme::palette_ctx(ctx);
    let spacing = Spacing::default();
    let mut submit = false;
    let mut cancel = false;

    egui::Window::new("Download all links with RDM")
        .collapsible(false)
        .resizable(true)
        .default_width(680.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!("{} downloadable links found", selection.links.len()));
            ui.add_space(spacing.sm);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(420.0)
                .show(ui, |ui| {
                    for item in &mut selection.links {
                        let text = match item.label.as_deref() {
                            Some(label) if label != item.url.as_str() => {
                                format!("{label}\n{}", item.url)
                            }
                            _ => item.url.clone(),
                        };
                        ui.checkbox(&mut item.checked, text)
                            .on_hover_text(item.url.as_str());
                    }
                });

            if let Some(error) = &selection.error {
                ui.add_space(spacing.sm);
                components::banner(ui, &palette, components::Level::Error, error);
            }

            ui.add_space(spacing.md);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Check All").clicked() {
                    for item in &mut selection.links {
                        item.checked = true;
                    }
                    selection.error = None;
                }
                if ui.button("Uncheck All").clicked() {
                    for item in &mut selection.links {
                        item.checked = false;
                    }
                    selection.error = None;
                }
                if components::primary_button(ui, "OK").clicked() {
                    submit = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });

    if cancel {
        actions.push(UiAction::CancelLinkSelection);
    } else if submit {
        actions.push(UiAction::SubmitLinkSelection);
    }
    actions
}
