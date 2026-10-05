//! In-app help (`F1` / the ⓘ button): what the states mean, the keyboard map
//! and the words rdm uses — the same content the tooltips carry, in one place,
//! so help is not hover-only (UX `RISK-UX-003`).
//!
//! The content comes from `ux::help_sections()`; the rendering layer only adds
//! geometry, so a new shortcut or legend entry appears here automatically and is
//! covered by the `ux.rs` unit tests.

use egui::{Context, RichText};

use crate::state::{GuiState, UiAction};
use crate::theme::{self, components, Sizes, Spacing};
use crate::ux;

pub fn show(ctx: &Context, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();
    if !state.show_help {
        return actions;
    }
    let palette = theme::palette_ctx(ctx);
    let spacing = Spacing::default();
    let sizes = Sizes::default();
    let mut close = false;

    egui::Window::new("Help — states, keyboard and words")
        .collapsible(false)
        .resizable(true)
        .default_width(sizes.details_width)
        .default_height(sizes.details_height)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (heading, lines) in ux::help_sections() {
                    components::section_title(ui, heading);
                    for line in lines {
                        ui.label(RichText::new(&line).color(palette.text));
                    }
                    ui.add_space(spacing.md);
                }
                ui.add_space(spacing.sm);
                if ui
                    .button("Close")
                    .on_hover_text("Close the help (Esc)")
                    .clicked()
                {
                    close = true;
                }
            });
        });

    if close {
        actions.push(UiAction::ToggleHelp);
    }
    actions
}
