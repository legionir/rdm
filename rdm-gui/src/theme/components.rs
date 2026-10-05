//! Reusable, token-driven widgets shared by every view.
//!
//! Views must not paint colours or pick sizes themselves; they call these
//! helpers (or read [`Palette`] / [`Sizes`] directly for one-off geometry).
//! Keeping the small vocabulary here is what makes the screens consistent:
//! a hint, a metadata line, a status chip and a row background look the same
//! in the table, the queue sidebar, the modals and the settings panel.

use egui::{Color32, Response, RichText, Ui};
use rdm::models::DownloadState;

use crate::theme::tokens::{state_glyph, Palette};

/// Severity of an inline message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warning,
    Error,
}

impl Level {
    pub fn color(self, palette: &Palette) -> Color32 {
        match self {
            Level::Info => palette.text_muted,
            Level::Warning => palette.warning,
            Level::Error => palette.danger,
        }
    }

    /// Map a captured log / event level string onto a banner severity.
    pub fn from_level(level: &str) -> Level {
        match level {
            "error" => Level::Error,
            "warn" => Level::Warning,
            _ => Level::Info,
        }
    }
}

/// Muted, italic hint text ("no events recorded for this download").
pub fn hint(ui: &mut Ui, palette: &Palette, text: impl Into<String>) -> Response {
    ui.add(egui::Label::new(
        RichText::new(text.into())
            .small()
            .italics()
            .color(palette.text_muted),
    ))
}

/// Secondary metadata: ids, byte counts, timestamps, speeds.
pub fn meta(ui: &mut Ui, palette: &Palette, text: impl Into<String>) -> Response {
    ui.add(
        egui::Label::new(
            RichText::new(text.into())
                .monospace()
                .small()
                .color(palette.text_muted),
        )
        .truncate(),
    )
}

/// Field name in the key/value grids and the settings form.
pub fn key_label(ui: &mut Ui, palette: &Palette, text: &str) -> Response {
    ui.add(egui::Label::new(
        RichText::new(text).small().color(palette.text_muted),
    ))
}

/// "Defaults for new downloads" style sub-heading.
pub fn section_title(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Label::new(RichText::new(text).strong()))
}

/// Status chip: glyph + state, coloured by the semantic palette and backed by
/// `surface_chip` so the label stays legible on every row surface.
///
/// The glyph means colour is never the only signal (WCAG 1.4.1).
pub fn status_chip(ui: &mut Ui, palette: &Palette, state: DownloadState) -> Response {
    let text = format!("{} {}", state_glyph(state), state);
    ui.add(
        egui::Label::new(
            RichText::new(text)
                .small()
                .color(palette.state_color(state))
                .background_color(palette.surface_chip),
        )
        .truncate(),
    )
}

/// The same glyph+label without the chip background, for modal headers.
pub fn state_text(palette: &Palette, state: DownloadState) -> RichText {
    RichText::new(format!("{} {}", state_glyph(state), state))
        .strong()
        .color(palette.state_color(state))
}

/// Inline message strip: form validation errors, warnings, informational text.
pub fn banner(ui: &mut Ui, palette: &Palette, kind: Level, text: &str) -> Response {
    ui.add(
        egui::Label::new(
            RichText::new(text)
                .small()
                .color(kind.color(palette))
                .background_color(palette.surface_alt),
        )
        .wrap(),
    )
}

/// Compact icon button used inside table rows, with a mandatory tooltip (the
/// accessible name for a glyph-only control).
pub fn icon_button(ui: &mut Ui, glyph: &str, tooltip: &str) -> bool {
    ui.small_button(glyph).on_hover_text(tooltip).clicked()
}

/// Primary action ("New download", "Start"); returns the response so callers
/// can still attach a tooltip.
pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
    ui.button(RichText::new(label).strong())
}

/// Destructive action ("Remove"); returns the response for tooltips.
pub fn danger_button(ui: &mut Ui, palette: &Palette, label: &str) -> Response {
    ui.button(RichText::new(label).color(palette.danger))
}

/// Row fill precedence: selected > hovered > zebra stripe > transparent.
///
/// Pure function so the precedence is unit-testable and stays identical in
/// the table and the queue sidebar.
pub fn row_background(
    palette: &Palette,
    selected: bool,
    hovered: bool,
    alternate: bool,
) -> Option<Color32> {
    if selected {
        Some(palette.surface_selected)
    } else if hovered {
        Some(palette.surface_hover)
    } else if alternate {
        Some(palette.zebra)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::tokens;

    #[test]
    fn row_background_precedence_is_stable() {
        for palette in [tokens::dark().palette, tokens::light().palette] {
            assert_eq!(
                row_background(&palette, true, true, true),
                Some(palette.surface_selected)
            );
            assert_eq!(
                row_background(&palette, false, true, true),
                Some(palette.surface_hover)
            );
            assert_eq!(row_background(&palette, false, false, true), Some(palette.zebra));
            assert_eq!(row_background(&palette, false, false, false), None);
        }
    }

    #[test]
    fn log_levels_map_to_banner_severities() {
        assert_eq!(Level::from_level("error"), Level::Error);
        assert_eq!(Level::from_level("warn"), Level::Warning);
        assert_eq!(Level::from_level("info"), Level::Info);
        assert_eq!(Level::from_level("debug"), Level::Info);
    }
}
