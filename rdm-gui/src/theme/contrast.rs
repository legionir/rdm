//! WCAG 2.x contrast math plus the *design contract* that binds every colour
//! token to the surfaces it is allowed to be painted on.
//!
//! The contract is data, not prose: [`CONTRACT`] lists every (role, surfaces,
//! minimum ratio) triple and the unit tests in this module assert it for both
//! themes. Adding a colour usage without adding it here makes the test suite
//! fail, which is the point — the numbers stay honest as the UI evolves.
//!
//! Thresholds come from WCAG 2.1:
//!
//! * SC 1.4.3 Contrast (Minimum) — normal text ≥ 4.5:1, large text ≥ 3:1
//! * SC 1.4.11 Non-text Contrast — focus indicators / UI components ≥ 3:1
//!
//! Decorative separators (`surface`/`border_subtle` hairlines, zebra stripes)
//! are intentionally *not* part of the contract: they convey no information
//! that is not also carried by text or by the selection fill.

use egui::Color32;

use super::tokens::Palette;

/// WCAG 2.1 SC 1.4.3 — normal-size text.
pub const AA_TEXT: f32 = 4.5;
/// WCAG 2.1 SC 1.4.3 — large text (≥ 24 px, or ≥ 18.66 px bold).
pub const AA_LARGE_TEXT: f32 = 3.0;
/// WCAG 2.1 SC 1.4.11 — non-text UI components and focus indicators.
pub const NON_TEXT: f32 = 3.0;

/// Every background a token may be painted on. Names match [`Palette`] fields
/// so the two can be cross-checked mechanically.
pub const SURFACES: [&str; 8] = [
    "surface",
    "surface_window",
    "surface_input",
    "surface_alt",
    "surface_raised",
    "surface_hover",
    "surface_selected",
    "surface_chip",
];

const ROW_SURFACES: &[&str] = &[
    "surface",
    "surface_window",
    "surface_input",
    "surface_alt",
    "surface_raised",
    "surface_hover",
    "surface_selected",
    "surface_chip",
];

/// (role, surfaces it may be used on, minimum contrast ratio).
pub const CONTRACT: &[(&str, &[&str], f32)] = &[
    ("text_primary", ROW_SURFACES, AA_TEXT),
    ("text_muted", ROW_SURFACES, AA_TEXT),
    ("success", ROW_SURFACES, AA_TEXT),
    ("accent", ROW_SURFACES, AA_TEXT),
    ("merging", ROW_SURFACES, AA_TEXT),
    ("warning", ROW_SURFACES, AA_TEXT),
    ("interrupted", ROW_SURFACES, AA_TEXT),
    ("danger", ROW_SURFACES, AA_TEXT),
];

/// A contract pair that does not reach its minimum ratio.
#[derive(Debug, Clone, PartialEq)]
pub struct Violation {
    pub role: String,
    pub surface: String,
    pub required: f32,
    pub actual: f32,
}

/// WCAG 2.x relative luminance of an sRGB colour.
pub fn relative_luminance(color: Color32) -> f32 {
    fn channel(v: u8) -> f32 {
        let c = v as f32 / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

/// WCAG 2.x contrast ratio, always ≥ 1.0 (identical colours → 1.0).
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Resolve a surface name from [`SURFACES`] to its colour.
pub fn surface_color(palette: &Palette, name: &str) -> Option<Color32> {
    match name {
        "surface" => Some(palette.surface),
        "surface_window" => Some(palette.surface_window),
        "surface_input" => Some(palette.surface_input),
        "surface_alt" => Some(palette.surface_alt),
        "surface_raised" => Some(palette.surface_raised),
        "surface_hover" => Some(palette.surface_hover),
        "surface_selected" => Some(palette.surface_selected),
        "surface_chip" => Some(palette.surface_chip),
        _ => None,
    }
}

/// Resolve a role name from [`CONTRACT`] to its colour.
pub fn role_color(palette: &Palette, name: &str) -> Option<Color32> {
    match name {
        "text_primary" => Some(palette.text_primary),
        "text_muted" => Some(palette.text_muted),
        "success" => Some(palette.success),
        "accent" => Some(palette.accent),
        "merging" => Some(palette.merging),
        "warning" => Some(palette.warning),
        "interrupted" => Some(palette.interrupted),
        "danger" => Some(palette.danger),
        _ => None,
    }
}

/// Every contract pair of `palette` that misses its minimum ratio.
pub fn violations(palette: &Palette) -> Vec<Violation> {
    let mut out = Vec::new();
    for (role, surfaces, required) in CONTRACT {
        let Some(fg) = role_color(palette, *role) else {
            out.push(Violation {
                role: (*role).to_string(),
                surface: "<unknown role>".to_string(),
                required: *required,
                actual: 0.0,
            });
            continue;
        };
        for surface in *surfaces {
            let Some(bg) = surface_color(palette, *surface) else {
                out.push(Violation {
                    role: (*role).to_string(),
                    surface: (*surface).to_string(),
                    required: *required,
                    actual: 0.0,
                });
                continue;
            };
            let actual = contrast_ratio(fg, bg);
            if actual + f32::EPSILON < *required {
                out.push(Violation {
                    role: (*role).to_string(),
                    surface: (*surface).to_string(),
                    required: *required,
                    actual,
                });
            }
        }
    }
    out
}

/// The worst text contrast of a palette (for the design spec / evidence).
pub fn worst_pair(palette: &Palette) -> (String, String, f32) {
    let mut worst = (String::new(), String::new(), f32::MAX);
    for (role, surfaces, _) in CONTRACT {
        let Some(fg) = role_color(palette, *role) else {
            continue;
        };
        for surface in *surfaces {
            let Some(bg) = surface_color(palette, *surface) else {
                continue;
            };
            let ratio = contrast_ratio(fg, bg);
            if ratio < worst.2 {
                worst = ((*role).to_string(), (*surface).to_string(), ratio);
            }
        }
    }
    worst
}

/// Is the focus/accent colour visible against every surface (SC 1.4.11)?
pub fn focus_is_visible(palette: &Palette) -> bool {
    SURFACES
        .iter()
        .filter_map(|s| surface_color(palette, s))
        .all(|bg| contrast_ratio(palette.accent, bg) >= NON_TEXT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::tokens;

    fn both() -> [Palette; 2] {
        [tokens::dark().palette, tokens::light().palette]
    }

    #[test]
    fn luminance_anchors_match_the_wcag_definition() {
        let ratio = contrast_ratio(Color32::BLACK, Color32::WHITE);
        assert!((ratio - 21.0).abs() < 0.01, "black/white = {ratio}");
        assert!((contrast_ratio(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 0.001);
    }

    #[test]
    fn every_declared_pair_meets_its_minimum_in_both_themes() {
        for palette in both() {
            let bad = violations(&palette);
            assert!(
                bad.is_empty(),
                "dark_mode={} contrast violations: {bad:#?}",
                palette.dark_mode
            );
        }
    }

    #[test]
    fn worst_text_pair_keeps_an_aa_margin() {
        for palette in both() {
            let (role, surface, ratio) = worst_pair(&palette);
            assert!(
                ratio >= AA_TEXT,
                "dark_mode={} worst pair {role} on {surface} = {ratio}",
                palette.dark_mode
            );
        }
    }

    #[test]
    fn focus_indicator_is_visible_on_every_surface() {
        for palette in both() {
            assert!(
                focus_is_visible(&palette),
                "focus ring invisible for dark_mode={}",
                palette.dark_mode
            );
        }
    }

    #[test]
    fn contract_names_all_resolve() {
        for palette in both() {
            for surface in SURFACES {
                assert!(
                    surface_color(&palette, surface).is_some(),
                    "unknown surface {surface}"
                );
            }
            for (role, _, _) in CONTRACT {
                assert!(
                    role_color(&palette, role).is_some(),
                    "unknown role {role}"
                );
            }
        }
    }
}
