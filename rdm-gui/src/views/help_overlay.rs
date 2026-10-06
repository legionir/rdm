//! Help — a real window (`F1` / the info button): what the states mean, the
//! keyboard map and the words rdm uses. The same content the tooltips carry, in
//! one place, so help is not hover-only (UX `RISK-UX-003`).
//!
//! ## Why it is a viewport and not an in-window modal
//!
//! It used to be an `egui::Window` floating inside the main window: no title
//! bar of its own, no ✕ (the one it drew rendered as an empty box), and it
//! disappeared behind the main window the moment that window had focus. It is
//! now a deferred viewport — an ordinary decorated window with the system's
//! close button in the top-right corner, moveable and resizeable like the
//! Settings window.
//!
//! ## Input
//!
//! `F1` still toggles it. The `Esc`/`F1` handling stays in the app loop, which
//! flips [`GuiState::show_help`]; this module turns that flip into a real
//! `ViewportCommand::Close`, and turns the window's own ✕ into the same state
//! flip, so both routes end in one place.
//!
//! The content comes from `ux::help_sections()`; the rendering layer only adds
//! geometry, so a new shortcut or legend entry appears here automatically and is
//! covered by the `ux.rs` unit tests.

use egui::{Context, RichText, ViewportBuilder, ViewportId};

use crate::state::{GuiState, UiAction};
use crate::theme::{self, components, Sizes, Spacing};
use crate::ux;

/// Show (or close) the help window.
pub fn show(ctx: &Context, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let id = ViewportId::from_hash_of("rdm-help-window");

    if !state.show_help {
        if HELP_SHOWN.with(|shown| shown.get()) {
            ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Close);
            HELP_SHOWN.with(|shown| shown.set(false));
        }
        return actions;
    }

    let palette = theme::palette_ctx(ctx);
    let spacing = Spacing::default();
    let sizes = Sizes::default();
    let dark = palette.dark_mode;
    let builder = ViewportBuilder::default()
        .with_title("rdm — Help")
        // A real window: the system draws the title bar and the ✕.
        .with_decorations(true)
        .with_resizable(true)
        .with_inner_size(sizes.details_default)
        .with_min_inner_size(sizes.details_min);

    ctx.show_viewport_deferred(id, builder, move |ctx, _class| {
        if ctx.style().visuals.dark_mode != dark {
            theme::install(ctx, dark);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(palette.surface))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .id_salt("help-window-scroll")
                    .show(ui, |ui| {
                        ui.add_space(spacing.xs);
                        for (heading, lines) in ux::help_sections() {
                            components::section_title(ui, heading);
                            for line in lines {
                                ui.label(RichText::new(&line).color(palette.text_primary));
                            }
                            ui.add_space(spacing.md);
                        }
                    });
            });
        // The window's own ✕ closes it. There is no in-window ✕ any more: the
        // system's title-bar button replaces the glyph that used to draw as an
        // empty box.
        if ctx.input(|i| i.viewport().close_requested()) {
            HELP_CLOSE_REQUESTED.with(|flag| flag.set(true));
        }
    });
    HELP_SHOWN.with(|shown| shown.set(true));

    if HELP_CLOSE_REQUESTED.with(|flag| flag.take()) {
        actions.push(UiAction::ToggleHelp);
    }
    actions
}

thread_local! {
    /// The help viewport is on screen, so a hide has to close it explicitly.
    static HELP_SHOWN: std::cell::Cell<bool> = std::cell::Cell::new(false);
    /// The window's own ✕ was used during this frame.
    static HELP_CLOSE_REQUESTED: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_content_is_renderable_and_not_empty() {
        let sections = ux::help_sections();
        assert!(!sections.is_empty(), "help has no sections");
        for (heading, lines) in sections {
            assert!(!heading.is_empty(), "a help section has no heading");
            assert!(!lines.is_empty(), "{heading:?} has no lines");
        }
    }

    #[test]
    fn help_states_the_window_and_the_close_route() {
        // Esc and F1 both land on `show_help`; this keeps the promise that the
        // window can always be closed from the keyboard as well as with the ✕.
        let shortcuts = ux::SHORTCUTS;
        assert!(
            shortcuts.iter().any(|(key, _what)| key.contains("Esc")),
            "Esc is no longer documented"
        );
        assert!(
            shortcuts
                .iter()
                .any(|(key, what)| format!("{key} {what}").to_lowercase().contains("help")),
            "the help shortcut is no longer documented"
        );
    }
}
