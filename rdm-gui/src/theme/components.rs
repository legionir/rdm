//! Reusable, token-driven widgets shared by every view.
//!
//! Views must not paint colours or pick sizes themselves; they call these
//! helpers (or read [`Palette`] / [`Sizes`] directly for one-off geometry).
//! Keeping the small vocabulary here is what makes the screens consistent:
//! a hint, a metadata line, a status chip and a row background look the same
//! in the table, the queue sidebar, the modals and the settings panel.

use egui::{Color32, FontId, Response, RichText, Sense, TextStyle, Ui, Vec2};
use rdm::models::DownloadState;

use crate::theme::icons::{self, Icon};
use crate::theme::tokens::{Palette, Spacing};

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

/// Status chip: state icon + state name, coloured by the semantic palette and
/// backed by `surface_chip` so the label stays legible on every row surface.
///
/// The icon means colour is never the only signal (WCAG 1.4.1). It is *painted*
/// (see [`crate::theme::icons`]), not a font glyph — the earlier glyph version
/// rendered as an empty box on systems whose fonts lack those codepoints.
pub fn status_chip(ui: &mut Ui, palette: &Palette, state: DownloadState) -> Response {
    let spacing = Spacing::default();
    let font = small_font(ui);
    let colour = palette.state_color(state);
    let galley = ui
        .painter()
        .layout_no_wrap(state.to_string(), font, colour);
    let icon = spacing.state_icon;
    let pad = Vec2::new(spacing.sm, spacing.xxs);
    let size = Vec2::new(
        icon + spacing.xs + galley.size().x + pad.x * 2.0,
        (icon + pad.y * 2.0).max(galley.size().y + pad.y * 2.0),
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, palette.surface_chip);
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + pad.x + icon / 2.0, rect.center().y),
        Vec2::splat(icon),
    );
    icons::paint(painter, Icon::state(state), icon_rect, colour);
    painter.galley(
        egui::pos2(icon_rect.right() + spacing.xs, rect.center().y - galley.size().y / 2.0),
        galley,
        colour,
    );
    response
}

/// The state name alone (no glyph) for places that build their own text.
pub fn state_text(palette: &Palette, state: DownloadState) -> RichText {
    RichText::new(state.to_string())
        .strong()
        .color(palette.state_color(state))
}

/// State icon + name in one row: modal headers wear the same mark as the chips.
pub fn state_heading(ui: &mut Ui, palette: &Palette, state: DownloadState) -> Response {
    let spacing = Spacing::default();
    let icon = spacing.state_icon * 1.3;
    let colour = palette.state_color(state);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(icon), Sense::hover());
        icons::paint(ui.painter(), Icon::state(state), rect, colour);
        ui.label(state_text(palette, state));
    })
    .response
}

/// The small text style, with a fallback for a context that never got a theme.
fn small_font(ui: &Ui) -> FontId {
    ui.style()
        .text_styles
        .get(&TextStyle::Small)
        .cloned()
        .unwrap_or_else(|| FontId::proportional(11.0))
}

/// A single-line text input with the standard geometry: same height as buttons,
/// combo boxes and the folder pickers, one place to change it.
pub fn text_edit(ui: &mut Ui, value: &mut String, width: f32, hint: &str) -> Response {
    let spacing = Spacing::default();
    // The margin only pads the text; the *height* is the shared control height,
    // because `Spacing::interact` sets `interact_size.y` (see `theme::install`).
    // Adding more vertical margin here is what made inputs taller than buttons.
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(width)
            .margin(egui::Margin::symmetric(spacing.sm, spacing.xs)),
    )
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
pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str) -> Response {
    let spacing = Spacing::default();
    let response = ui.add_sized(
        Vec2::splat(spacing.icon_button_side),
        egui::Button::new(""),
    );
    paint_icon(ui, &response, icon, spacing.icon_button_side);
    response.on_hover_text(tooltip)
}

/// A button that carries an icon and a label, both inside the control height.
///
/// egui lays the label out itself, so the icon is painted into a small leading
/// gap reserved with spaces — that keeps one code path for the frame, the
/// hover state and the height, which is what makes the row uniform.
pub fn icon_text_button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    tooltip: &str,
) -> Response {
    let spacing = Spacing::default();
    let response = ui.add(egui::Button::new(format!("   {label}")).selected(selected));
    let icon_rect = egui::Rect::from_min_size(
        response.rect.left_top() + Vec2::new(spacing.sm, spacing.xs),
        Vec2::splat(spacing.control_height - spacing.xs * 2.0),
    );
    let colour = ui.visuals().text_color();
    icons::paint(ui.painter(), icon, icon_rect, colour);
    response.on_hover_text(tooltip)
}

/// Paint `icon` inside a widget's rect, in the colour the widget's text uses.
fn paint_icon(ui: &Ui, response: &Response, icon: Icon, side: f32) {
    let inset = (side * 0.22).max(4.0);
    let colour = if response.hovered() {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().text_color()
    };
    icons::paint(ui.painter(), icon, response.rect.shrink(inset), colour);
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
