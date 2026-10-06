//! Reusable, token-driven widgets shared by every view.
//!
//! Views must not paint colours or pick sizes themselves; they call these
//! helpers (or read [`Palette`] / [`Sizes`] directly for one-off geometry).
//! Keeping the small vocabulary here is what makes the screens consistent:
//! a hint, a metadata line, a status chip and a row background look the same
//! in the table, the queue sidebar, the modals and the settings panel.

use egui::{Color32, FontId, Pos2, Rect, Response, RichText, Sense, TextStyle, Ui, Vec2};
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

/// The font a `Button` lays its own label out with.
///
/// Used to measure the label of a labelled button, so the control is sized for
/// the text that will actually be painted into it.
fn button_font(ui: &Ui) -> FontId {
    ui.style()
        .text_styles
        .get(&TextStyle::Button)
        .cloned()
        .unwrap_or_else(|| FontId::proportional(12.5))
}

/// A single-line text input with the standard geometry: same height as buttons,
/// combo boxes and the folder pickers, one place to change it.
///
/// The reply's `rect` is the **inner** text rect: `TextEdit` shrinks it by the
/// margin on purpose (egui 0.29 `text_edit/builder.rs:416`). What the user sees
/// and what lines up with the next control is the frame, and that is the full
/// control height; a caller that needs that rect wraps the call in a
/// `ui.scope(…)` (the layout test does) rather than trusting `Response::rect`.
pub fn text_edit(ui: &mut Ui, value: &mut String, width: f32, hint: &str) -> Response {
    let spacing = Spacing::default();
    // The margin only pads the text; the *height* is the shared control height,
    // because `Spacing::interact` sets `interact_size.y` (see `theme::install`).
    // Adding more vertical margin here is what made inputs taller than buttons.
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(width)
            // The height is the shared *control* height, not “one text row plus
            // margins”: the row alone is ≈22.4 pt, so without this the input
            // sits shorter than the button beside it — round 3’s “standardise
            // the heights” report, and the last red test in
            // `theme::tests::every_control_in_a_row_is_control_height_tall`.
            .min_size(egui::vec2(0.0, spacing.control_height))
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
/// egui reserves room in a `Button` for its *text*; a painted icon (see
/// [`crate::theme::icons`]) has no advance width for it to reserve. The earlier
/// version faked the icon's slot with three literal spaces and pinned the icon
/// to a fixed inset, so how much room the two had between them was a property of
/// the font — on the shipped font the label's ink began *inside* the icon. That
/// is the round-5 report, “the icon is glued to the text”.
///
/// The control is now sized by [`icon_text_layout`] from the measured label and
/// the spacing tokens, and the icon and the label are painted into it: the gap
/// between them is [`Spacing::icon_gap`] — arithmetic, not typography. egui
/// still draws the frame, the hover state and the click.
pub fn icon_text_button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    tooltip: &str,
) -> Response {
    let spacing = Spacing::default();
    let colour = ui.visuals().text_color();
    // Measured, never estimated: the label is laid out with the very style a
    // `Button` would have used for it, and the button is sized to hold it.
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), button_font(ui), colour);
    let layout = icon_text_layout(galley.size(), &spacing);

    let response = ui.add(
        egui::Button::new("")
            .selected(selected)
            .min_size(layout.size),
    );
    // The paint is the whole widget here, so the label has to be reported for
    // anything that reads the widget tree (accessibility, tests): an empty
    // `Button` on its own carries no name.
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });

    let origin = response.rect.min.to_vec2();
    icons::paint(ui.painter(), icon, layout.icon_rect.translate(origin), colour);
    ui.painter().galley(layout.text_pos + origin, galley, colour);
    response.on_hover_text(tooltip)
}

/// Where the icon, the gap and the label of a labelled button go.
///
/// Pure arithmetic on [`Spacing`] — the label comes in as its *measured* size —
/// so the numbers the user sees are the numbers the unit tests measure, and the
/// gap can never drift with a font.
#[derive(Debug, Clone, Copy)]
pub struct IconTextLayout {
    /// The whole control: padding + icon + gap + label + padding.
    pub size: Vec2,
    /// The icon, relative to the control's top-left corner.
    pub icon_rect: Rect,
    /// The label's top-left corner, relative to the same corner.
    pub text_pos: Pos2,
}

impl IconTextLayout {
    /// The room between the icon and the label — the number round 5 was about.
    ///
    /// A named method so a test asserts the contract itself instead of
    /// re-deriving it by hand.
    pub fn icon_label_gap(&self) -> f32 {
        self.text_pos.x - self.icon_rect.right()
    }
}

/// Lay out one labelled button.
pub fn icon_text_layout(label_size: Vec2, spacing: &Spacing) -> IconTextLayout {
    let icon = spacing.icon_in_button;
    let pad = spacing.button.x;
    let gap = spacing.icon_gap;
    IconTextLayout {
        size: Vec2::new(pad + icon + gap + label_size.x + pad, spacing.control_height),
        icon_rect: Rect::from_min_size(
            Pos2::new(pad, (spacing.control_height - icon) * 0.5),
            Vec2::splat(icon),
        ),
        // `max(0.0)`: a label taller than the control starts at the top edge
        // instead of hanging out of it.
        text_pos: Pos2::new(
            pad + icon + gap,
            ((spacing.control_height - label_size.y) * 0.5).max(0.0),
        ),
    }
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
    fn the_icon_never_touches_the_label() {
        // Round 5: the label's ink began inside the icon, because the slot was
        // reserved with literal spaces. The gap is arithmetic now, so the
        // contract can be asserted for every label width a toolbar can hold.
        let spacing = Spacing::default();
        for width in [0.0, 12.0, 34.0, 78.0, 156.0] {
            let layout = icon_text_layout(Vec2::new(width, 15.0), &spacing);
            assert!(
                layout.icon_label_gap() >= spacing.icon_gap,
                "assertion: a {width} pt label leaves {:.2} pt between the icon and the text, \
                 expected at least {:.2} pt",
                layout.icon_label_gap(),
                spacing.icon_gap
            );
        }
    }

    #[test]
    fn the_icon_and_the_label_share_the_control_height() {
        let spacing = Spacing::default();
        let label = Vec2::new(80.0, 15.0);
        let layout = icon_text_layout(label, &spacing);
        let middle = spacing.control_height * 0.5;

        assert!(
            (layout.size.y - spacing.control_height).abs() < 0.01,
            "the labelled button is {:.2} pt tall, not the control height {:.2} pt",
            layout.size.y,
            spacing.control_height
        );
        // Both halves sit on the control's midline, inside its edges.
        assert!((layout.icon_rect.center().y - middle).abs() < 0.01);
        assert!((layout.text_pos.y + label.y * 0.5 - middle).abs() < 0.01);
        assert!(layout.icon_rect.left() >= 0.0);
        assert!(
            layout.text_pos.x + label.x <= layout.size.x - spacing.button.x + 0.01,
            "the label overflows the control's trailing padding"
        );
    }

    #[test]
    fn log_levels_map_to_banner_severities() {
        assert_eq!(Level::from_level("error"), Level::Error);
        assert_eq!(Level::from_level("warn"), Level::Warning);
        assert_eq!(Level::from_level("info"), Level::Info);
        assert_eq!(Level::from_level("debug"), Level::Info);
    }
}
