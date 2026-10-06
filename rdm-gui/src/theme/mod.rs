//! Design-token layer of the rdm desktop UI.
//!
//! * [`tokens`] — palette, spacing scale, radii, typography, sizes, breakpoints
//! * [`contrast`] — WCAG maths plus the role→surface contract the palette obeys
//! * [`components`] — the small widget vocabulary every screen reuses
//! * [`icons`] — drawn icons (no font glyphs, so nothing can render as a box)
//!
//! Views read tokens through [`palette_of`] / [`palette_ctx`]; only [`install`]
//! touches `egui::Visuals`, so a theme change is a single edit here.
//!
//! The unit test at the bottom of this file is a *design guard*: it fails the
//! build if a view starts hardcoding colour literals again.

pub mod components;
pub mod contrast;
pub mod icons;
pub mod tokens;

pub use components::Level;
pub use icons::Icon;
pub use tokens::{Breakpoints, LayoutMode, Palette, Radii, Sizes, Spacing, Table, Tokens, Typography};

use egui::{Context, FontFamily, FontId, TextStyle};
use std::collections::BTreeMap;

/// All tokens for one theme.
pub fn tokens(dark: bool) -> Tokens {
    Tokens::for_dark(dark)
}

/// The palette for one theme.
pub fn palette(dark: bool) -> Palette {
    Tokens::for_dark(dark).palette
}

/// The palette of the currently installed style.
pub fn palette_ctx(ctx: &Context) -> Palette {
    palette(ctx.style().visuals.dark_mode)
}

/// The palette of the UI the widget is drawn into.
pub fn palette_of(ui: &egui::Ui) -> Palette {
    palette(ui.visuals().dark_mode)
}

/// Apply the theme: palette, spacing, radii-independent global paddings and the
/// typography scale. Called whenever the user flips the dark/light switch.
pub fn install(ctx: &Context, dark: bool) {
    let t = tokens(dark);
    let p = t.palette;

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = t.spacing.item;
    style.spacing.button_padding = t.spacing.button;
    style.spacing.interact_size = t.spacing.interact;
    style.text_styles = text_styles(&t.typography);
    style.visuals = visuals(&p);
    ctx.set_style(style);
}

/// The typography scale applied to egui's named text styles.
pub fn text_styles(t: &Typography) -> BTreeMap<TextStyle, FontId> {
    let mut styles = BTreeMap::new();
    styles.insert(TextStyle::Small, FontId::new(t.small, FontFamily::Proportional));
    styles.insert(TextStyle::Body, FontId::new(t.body, FontFamily::Proportional));
    styles.insert(
        TextStyle::Button,
        FontId::new(t.button, FontFamily::Proportional),
    );
    styles.insert(
        TextStyle::Heading,
        FontId::new(t.heading, FontFamily::Proportional),
    );
    styles.insert(
        TextStyle::Monospace,
        FontId::new(t.monospace, FontFamily::Monospace),
    );
    styles
}

/// Map the token palette onto egui's `Visuals`.
///
/// Only documented, contract-checked surfaces are written: the fills come from
/// the palette, so every text-on-widget pair is covered by the contrast tests
/// instead of relying on egui's defaults.
fn visuals(p: &Palette) -> egui::Visuals {
    let mut v = if p.dark_mode {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = p.surface;
    v.window_fill = p.surface_window;
    v.extreme_bg_color = p.surface_input;
    v.faint_bg_color = p.surface_alt;
    v.selection.bg_fill = p.surface_selected;
    v.selection.stroke = egui::Stroke::new(1.0_f32, p.text_primary);
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.danger;

    let hairline = egui::Stroke::new(1.0_f32, p.border_subtle);
    let text = egui::Stroke::new(1.0_f32, p.text_primary);

    v.widgets.noninteractive.bg_fill = p.surface;
    v.widgets.noninteractive.weak_bg_fill = p.surface;
    v.widgets.noninteractive.fg_stroke = text;
    v.widgets.noninteractive.bg_stroke = hairline;

    v.widgets.inactive.bg_fill = p.surface_raised;
    v.widgets.inactive.weak_bg_fill = p.surface_raised;
    v.widgets.inactive.fg_stroke = text;
    v.widgets.inactive.bg_stroke = hairline;

    v.widgets.hovered.bg_fill = p.surface_hover;
    v.widgets.hovered.weak_bg_fill = p.surface_hover;
    v.widgets.hovered.fg_stroke = text;
    v.widgets.hovered.bg_stroke = hairline;

    v.widgets.active.bg_fill = p.surface_selected;
    v.widgets.active.weak_bg_fill = p.surface_selected;
    v.widgets.active.fg_stroke = text;
    v.widgets.active.bg_stroke = hairline;

    v.widgets.open.bg_fill = p.surface_window;
    v.widgets.open.weak_bg_fill = p.surface_alt;
    v.widgets.open.fg_stroke = text;
    v.widgets.open.bg_stroke = hairline;

    v
}

/// Cap sidebars so a long path / infinite-width widget cannot eat the list.
pub fn sidebar_max_width(window_width: f32) -> f32 {
    let s = Sizes::default();
    (window_width * s.sidebar_max_ratio).clamp(s.sidebar_max_floor, s.sidebar_max_abs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every file that renders UI. Colour literals are only allowed to appear
    /// inside the token/component layer, which is what makes "no hardcoded
    /// values" checkable instead of aspirational.
    const VIEWS: &[(&str, &str)] = &[
        ("app.rs", include_str!("../app.rs")),
        ("views/add_download.rs", include_str!("../views/add_download.rs")),
        ("views/details_modal.rs", include_str!("../views/details_modal.rs")),
        ("views/download_list.rs", include_str!("../views/download_list.rs")),
        ("views/footer.rs", include_str!("../views/footer.rs")),
        ("views/queue_sidebar.rs", include_str!("../views/queue_sidebar.rs")),
        ("views/settings_view.rs", include_str!("../views/settings_view.rs")),
        ("views/toolbar.rs", include_str!("../views/toolbar.rs")),
    ];

    const FORBIDDEN: [&str; 8] = [
        "Color32::from_rgb",
        "Color32::from_rgba",
        "Color32::from_gray",
        "Color32::from_white_alpha",
        "Color32::from_black_alpha",
        "Color32::GRAY",
        "Color32::RED",
        "Color32::WHITE",
    ];


    /// The sizes the user complained about, measured for real.
    ///
    /// egui lays widgets out without a window, so this test runs the *actual*
    /// layout pass: a button, the square icon button, a text input, a combo box
    /// and the labelled button must all come out at [`Spacing::control_height`].
    /// That is the guarantee behind “standardise the heights” — a regression here
    /// is one a reviewer would otherwise only see on a desktop.
    #[test]
    fn every_control_in_a_row_is_control_height_tall() {
        let ctx = Context::default();
        install(&ctx, true);

        let expected = Spacing::default().control_height;
        // The token and the style the widgets actually read must agree.
        assert!(
            (ctx.style().spacing.interact_size.y - expected).abs() < 0.01,
            "interact_size.y is {}, expected the control height {expected}",
            ctx.style().spacing.interact_size.y
        );

        let mut text = String::new();
        let mut heights: Vec<(&str, f32)> = Vec::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                heights.push(("button", ui.button("Clear").rect.height()));
                heights.push((
                    "icon button",
                    components::icon_button(ui, Icon::Close, "clear").rect.height(),
                ));
                heights.push((
                    "text input",
                    components::text_edit(ui, &mut text, 120.0, "hint")
                        .rect
                        .height(),
                ));
                // `ComboBox` is not a `Widget` in egui 0.29 — it is shown, and
                // the height to compare is its button's.
                heights.push((
                    "combo box",
                    egui::ComboBox::from_id_salt("combo")
                        .selected_text("all states")
                        .show_ui(ui, |_| {})
                        .response
                        .rect
                        .height(),
                ));
                heights.push((
                    "labelled button",
                    components::icon_text_button(ui, Icon::Plus, "New download", false, "t")
                        .rect
                        .height(),
                ));
            });
        });

        for (name, height) in heights {
            assert!(
                (height - expected).abs() < 1.0,
                "{name} is {height:.1} pt, expected the shared control height {expected:.1} pt"
            );
        }
    }

    #[test]
    fn views_do_not_hardcode_colours() {
        for (name, source) in VIEWS {
            for needle in FORBIDDEN {
                assert!(
                    !source.contains(needle),
                    "{name} hardcodes a colour via `{needle}` — use theme tokens"
                );
            }
        }
    }

    #[test]
    fn every_view_uses_the_token_layer() {
        for (name, source) in VIEWS {
            assert!(
                source.contains("theme::"),
                "{name} does not reference the theme module"
            );
        }
    }

    #[test]
    fn sidebar_never_eats_the_window() {
        assert_eq!(sidebar_max_width(2000.0), 400.0);
        assert_eq!(sidebar_max_width(1000.0), 400.0);
        assert_eq!(sidebar_max_width(800.0), 320.0);
        assert_eq!(sidebar_max_width(400.0), 260.0);
    }

    #[test]
    fn installing_a_theme_is_reversible() {
        // Both directions produce the palette the mode asks for.
        for dark in [true, false] {
            assert_eq!(palette(dark).dark_mode, dark);
            assert_eq!(tokens(dark).palette.dark_mode, dark);
        }
    }
}
