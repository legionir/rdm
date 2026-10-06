//! Design tokens for the rdm desktop UI.
//!
//! Everything visual that the views need lives here: the semantic palette
//! (per theme), the spacing scale, corner radii, typography, component sizes
//! and the responsive breakpoints. Views must consume these tokens instead of
//! literals — `theme::mod` has a unit test that fails if a view file starts
//! hardcoding colours again.
//!
//! Colour choices are constrained by [`crate::theme::contrast::CONTRACT`]:
//! every role below reaches at least WCAG AA (4.5:1) on every surface it is
//! used on, in both themes (worst pair ≥ 5.0:1, see the design spec).

use egui::{Color32, Vec2};
use rdm::models::{ChunkStatus, DownloadState};

/// Palette + geometry for one theme.
#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub palette: Palette,
    pub spacing: Spacing,
    pub radii: Radii,
    pub typography: Typography,
    pub sizes: Sizes,
    pub breakpoints: Breakpoints,
}

impl Tokens {
    /// Tokens for the requested theme; `dark_mode` is the same flag the
    /// palette carries, so the two can never drift apart.
    pub fn for_dark(dark_mode: bool) -> Self {
        if dark_mode {
            dark()
        } else {
            light()
        }
    }
}

/// The dark theme tokens.
pub fn dark() -> Tokens {
    Tokens {
        palette: dark_palette(),
        spacing: Spacing::default(),
        radii: Radii::default(),
        typography: Typography::default(),
        sizes: Sizes::default(),
        breakpoints: Breakpoints::default(),
    }
}

/// The light theme tokens.
pub fn light() -> Tokens {
    Tokens {
        palette: light_palette(),
        spacing: Spacing::default(),
        radii: Radii::default(),
        typography: Typography::default(),
        sizes: Sizes::default(),
        breakpoints: Breakpoints::default(),
    }
}

// ---------------------------------------------------------------- palette

/// Semantic colours. Fields are named after their *role*, never after a hue,
/// so a theme swap is a data change and not a view change.
///
/// `surface*` fields are the backgrounds the other roles are measured against;
/// the character of the palette (dark on light vs light on dark) is carried by
/// [`Palette::dark_mode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub dark_mode: bool,
    /// Panel / table background (egui `panel_fill`).
    pub surface: Color32,
    /// Floating windows, modals, inputs' resting fill (`window_fill`).
    pub surface_window: Color32,
    /// Text-edit / code background (`extreme_bg_color`).
    pub surface_input: Color32,
    /// Raised rows, zebra stripes, chips (`faint_bg_color`).
    pub surface_alt: Color32,
    /// Resting fill of interactive controls (buttons, combo boxes).
    pub surface_raised: Color32,
    /// Hovered row or widget fill.
    pub surface_hover: Color32,
    /// Selected row / selected widget fill.
    pub surface_selected: Color32,
    /// Status-chip background.
    pub surface_chip: Color32,
    /// Primary text: filenames, values, headings.
    pub text_primary: Color32,
    /// Secondary text: ids, metadata, hints, timestamps.
    pub text_muted: Color32,
    /// Completed / verified.
    pub success: Color32,
    /// Running transfer, focus ring, primary action, links.
    pub accent: Color32,
    /// Merging (post-processing) state.
    pub merging: Color32,
    /// Waiting / queued / warning.
    pub warning: Color32,
    /// Interrupted (resumable) state.
    pub interrupted: Color32,
    /// Failed / destructive / error text.
    pub danger: Color32,
    /// Decorative hairlines (table header rule, separators). Exempt from the
    /// contrast contract — it carries no information of its own.
    pub border_subtle: Color32,
    /// Pre-computed zebra tint over [`Palette::surface`] (decorative).
    pub zebra: Color32,
}

impl Palette {
    /// Colour for a download state (glyph + label + chip text).
    pub fn state_color(&self, state: DownloadState) -> Color32 {
        match state {
            DownloadState::Completed => self.success,
            DownloadState::Running => self.accent,
            DownloadState::Merging => self.merging,
            DownloadState::Queued => self.warning,
            DownloadState::Paused => self.text_muted,
            DownloadState::Interrupted => self.interrupted,
            DownloadState::Failed => self.danger,
            DownloadState::Cancelled => self.text_muted,
        }
    }

    /// Colour for a chunk row of the details modal.
    pub fn chunk_color(&self, status: ChunkStatus) -> Color32 {
        match status {
            ChunkStatus::Completed => self.success,
            ChunkStatus::Active => self.accent,
            ChunkStatus::Failed => self.danger,
            ChunkStatus::Pending => self.text_muted,
        }
    }

    /// Colour for a log / event level.
    pub fn log_color(&self, level: &str) -> Color32 {
        match level {
            "error" => self.danger,
            "warn" => self.warning,
            "debug" | "trace" => self.text_muted,
            _ => self.text_primary,
        }
    }

    /// The default text colour (`None` = inherit the theme's primary text).
    pub fn status_color(&self, is_error: bool) -> Color32 {
        if is_error {
            self.danger
        } else {
            self.text_muted
        }
    }
}

/// Dark palette. Surfaces are verified in [`crate::theme::contrast`].
fn dark_palette() -> Palette {
    let surface = Color32::from_rgb(0x1B, 0x1B, 0x1B);
    let surface_window = Color32::from_rgb(0x1F, 0x1F, 0x1F);
    let surface_input = Color32::from_rgb(0x11, 0x11, 0x11);
    let surface_alt = Color32::from_rgb(0x24, 0x24, 0x24);
    let surface_raised = Color32::from_rgb(0x33, 0x33, 0x33);
    let surface_hover = Color32::from_rgb(0x3A, 0x3A, 0x3A);
    let surface_selected = Color32::from_rgb(0x1E, 0x3A, 0x5F);
    let surface_chip = Color32::from_rgb(0x24, 0x24, 0x24);
    let text_primary = Color32::from_rgb(0xEC, 0xED, 0xEF);
    let text_muted = Color32::from_rgb(0xA8, 0xAE, 0xB8);
    let success = Color32::from_rgb(0x4A, 0xDE, 0x80);
    let accent = Color32::from_rgb(0x7C, 0xB3, 0xFF);
    let merging = Color32::from_rgb(0xC4, 0xB5, 0xFD);
    let warning = Color32::from_rgb(0xFB, 0xBF, 0x24);
    let interrupted = Color32::from_rgb(0xFD, 0xBA, 0x74);
    let danger = Color32::from_rgb(0xFF, 0x8A, 0x8A);
    let border_subtle = Color32::from_rgb(0x3A, 0x3F, 0x47);
    let zebra = Color32::from_rgb(0x24, 0x24, 0x24);
    Palette {
        dark_mode: true,
        surface,
        surface_window,
        surface_input,
        surface_alt,
        surface_raised,
        surface_hover,
        surface_selected,
        surface_chip,
        text_primary,
        text_muted,
        success,
        accent,
        merging,
        warning,
        interrupted,
        danger,
        border_subtle,
        zebra,
    }
}

/// Light palette. Surfaces are verified in [`crate::theme::contrast`].
fn light_palette() -> Palette {
    let surface = Color32::from_rgb(0xF8, 0xF8, 0xF8);
    let surface_window = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    let surface_input = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    let surface_alt = Color32::from_rgb(0xF0, 0xF0, 0xF0);
    let surface_raised = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    let surface_hover = Color32::from_rgb(0xE6, 0xE6, 0xE6);
    let surface_selected = Color32::from_rgb(0xD8, 0xEB, 0xFF);
    let surface_chip = Color32::from_rgb(0xF0, 0xF0, 0xF0);
    let text_primary = Color32::from_rgb(0x1A, 0x1D, 0x21);
    let text_muted = Color32::from_rgb(0x55, 0x5B, 0x63);
    let success = Color32::from_rgb(0x0F, 0x6B, 0x33);
    let accent = Color32::from_rgb(0x1D, 0x4E, 0xD8);
    let merging = Color32::from_rgb(0x6D, 0x28, 0xD9);
    let warning = Color32::from_rgb(0x7C, 0x4A, 0x0B);
    let interrupted = Color32::from_rgb(0x9A, 0x34, 0x12);
    let danger = Color32::from_rgb(0xB9, 0x1C, 0x1C);
    let border_subtle = Color32::from_rgb(0xD5, 0xD8, 0xDC);
    let zebra = Color32::from_rgb(0xF0, 0xF0, 0xF0);
    Palette {
        dark_mode: false,
        surface,
        surface_window,
        surface_input,
        surface_alt,
        surface_raised,
        surface_hover,
        surface_selected,
        surface_chip,
        text_primary,
        text_muted,
        success,
        accent,
        merging,
        warning,
        interrupted,
        danger,
        border_subtle,
        zebra,
    }
}

// ---------------------------------------------------------------- geometry

/// 4-px-based spacing scale. Values are the ones the UI already shipped with,
/// so token adoption is behaviour-preserving.
#[derive(Debug, Clone, Copy)]
pub struct Spacing {
    pub xxs: f32,
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
    /// Global `item_spacing`.
    pub item: Vec2,
    /// Height every interactive control shares: buttons, icon buttons, text
    /// inputs, combo boxes and the folder pickers. One number, one row height —
    /// the mix of `small_button`, default padding and per-widget margins is what
    /// made the toolbar look ragged.
    pub control_height: f32,
    /// Side of a square icon button (equals [`Spacing::control_height`]).
    pub icon_button_side: f32,
    /// State icon inside a status chip.
    pub state_icon: f32,
    /// Global `button_padding`.
    pub button: Vec2,
    /// Global `interact_size`.
    pub interact: Vec2,
    /// Table row height (the column padding lives in [`Table`]).
    pub row_height: f32,
    pub icon_gap: f32,
    /// Activity spinner next to the "running here" counter.
    pub spinner: f32,
}

impl Default for Spacing {
    fn default() -> Self {
        Spacing {
            xxs: 2.0,
            xs: 4.0,
            sm: 6.0,
            md: 8.0,
            lg: 10.0,
            xl: 12.0,
            xxl: 24.0,
            item: Vec2::new(8.0, 8.0),
            control_height: 24.0,
            icon_button_side: 24.0,
            state_icon: 12.0,
            // Padding is chosen so text + padding never exceeds the control
            // height; the height itself comes from `interact_size`.
            button: Vec2::new(10.0, 4.0),
            interact: Vec2::new(44.0, 24.0),
            row_height: 42.0,
            icon_gap: 5.0,
            spinner: 14.0,
        }
    }
}

/// Corner radii.
#[derive(Debug, Clone, Copy)]
pub struct Radii {
    pub sm: f32,
    pub md: f32,
}

impl Default for Radii {
    fn default() -> Self {
        Radii { sm: 2.0, md: 4.0 }
    }
}

/// Font sizes applied to egui's text styles.
#[derive(Debug, Clone, Copy)]
pub struct Typography {
    pub body: f32,
    pub small: f32,
    pub button: f32,
    pub heading: f32,
    pub monospace: f32,
}

impl Default for Typography {
    fn default() -> Self {
        Typography {
            body: 12.5,
            // Was egui's 9.0; 11.0 is the smallest size that stays comfortably
            // legible for the metadata columns and log lines.
            small: 11.0,
            button: 12.5,
            heading: 18.0,
            monospace: 12.0,
        }
    }
}

/// Fixed component sizes (windows, panels, columns, bars).
#[derive(Debug, Clone, Copy)]
pub struct Sizes {
    pub window_default: [f32; 2],
    pub window_min: [f32; 2],
    pub sidebar_default: f32,
    pub sidebar_queue_min: f32,
    pub sidebar_settings_min: f32,
    pub sidebar_max_ratio: f32,
    pub sidebar_max_floor: f32,
    pub sidebar_max_abs: f32,
    pub footer_default: f32,
    pub footer_min: f32,
    pub status_reserve: f32,
    pub status_height: f32,
    pub status_min: f32,
    pub search_width: f32,
    pub filter_width: f32,
    pub form_width: f32,
    pub form_label_width: f32,
    pub form_spacing: [f32; 2],
    pub edit_wide: f32,
    pub edit_medium: f32,
    pub edit_narrow: f32,
    pub picker_reserve: f32,
    pub picker_min: f32,
    pub details_default: [f32; 2],
    pub details_min: [f32; 2],
    pub progress_inset: f32,
    pub progress_min: f32,
    pub progress_chunk: f32,
    /// Visible rows of the JSON pane in the details modal.
    pub json_rows: usize,
    /// Key/value grid of the details modal.
    pub details_grid_spacing: [f32; 2],
    pub details_grid_key_width: f32,
    /// Chunk table of the details modal.
    pub chunk_grid_spacing: [f32; 2],
    pub table: Table,
}

/// Table geometry. `file_ideal`/`file_min` bound the flexible FILE column;
/// the rest are fixed widths managed by `views::download_list::Columns`.
#[derive(Debug, Clone, Copy)]
pub struct Table {
    /// Horizontal padding inside a row, and the gap between two columns.
    pub pad: f32,
    pub gap: f32,
    pub state: f32,
    pub file_ideal: f32,
    pub file_min: f32,
    pub id: f32,
    pub conns: f32,
    pub added: f32,
    pub progress: f32,
    pub progress_compact: f32,
    pub size: f32,
    pub speed: f32,
    pub eta: f32,
    pub actions: f32,
    pub header_height: f32,
    pub header_inset: f32,
    pub row_inset: f32,
    pub accent: f32,
}

impl Default for Sizes {
    fn default() -> Self {
        Sizes {
            window_default: [1180.0, 760.0],
            window_min: [860.0, 560.0],
            sidebar_default: 340.0,
            sidebar_queue_min: 240.0,
            sidebar_settings_min: 260.0,
            sidebar_max_ratio: 0.4,
            sidebar_max_floor: 260.0,
            sidebar_max_abs: 400.0,
            footer_default: 220.0,
            footer_min: 90.0,
            status_reserve: 260.0,
            status_height: 16.0,
            status_min: 80.0,
            search_width: 220.0,
            filter_width: 110.0,
            form_width: 580.0,
            form_label_width: 120.0,
            form_spacing: [12.0, 10.0],
            edit_wide: 380.0,
            edit_medium: 330.0,
            edit_narrow: 160.0,
            picker_reserve: 52.0,
            picker_min: 120.0,
            details_default: [780.0, 540.0],
            details_min: [500.0, 360.0],
            progress_inset: 6.0,
            progress_min: 40.0,
            progress_chunk: 120.0,
            json_rows: 18,
            details_grid_spacing: [14.0, 8.0],
            details_grid_key_width: 130.0,
            chunk_grid_spacing: [10.0, 6.0],
            table: Table::default(),
        }
    }
}

impl Default for Table {
    fn default() -> Self {
        Table {
            pad: 8.0,
            gap: 8.0,
            state: 82.0,
            file_ideal: 140.0,
            file_min: 80.0,
            id: 84.0,
            conns: 78.0,
            added: 86.0,
            progress: 150.0,
            progress_compact: 90.0,
            size: 106.0,
            speed: 72.0,
            eta: 58.0,
            actions: 158.0,
            header_height: 24.0,
            header_inset: 2.0,
            row_inset: 3.0,
            accent: 3.0,
        }
    }
}

/// Responsive breakpoints (logical pixels of the available width).
#[derive(Debug, Clone, Copy)]
pub struct Breakpoints {
    /// Below this the UI is a "compact" layout.
    pub compact: f32,
    /// Below this the UI is "regular" (all optional table columns fit).
    pub regular: f32,
}

impl Default for Breakpoints {
    fn default() -> Self {
        Breakpoints {
            compact: 720.0,
            regular: 1024.0,
        }
    }
}

/// Layout class derived from the available width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Narrow window: optional columns and help text are dropped first.
    Compact,
    /// Normal window.
    Regular,
    /// Wide window: every column and hint is shown.
    Wide,
}

impl Breakpoints {
    pub fn mode(&self, width: f32) -> LayoutMode {
        if width < self.compact {
            LayoutMode::Compact
        } else if width < self.regular {
            LayoutMode::Regular
        } else {
            LayoutMode::Wide
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ALL_STATES;

    #[test]
    fn every_download_state_has_a_colour_and_a_glyph() {
        for state in ALL_STATES {
            for palette in [dark_palette(), light_palette()] {
                let color = palette.state_color(state);
                assert_ne!(
                    color,
                    Color32::TRANSPARENT,
                    "state {state} has no colour in dark_mode={}",
                    palette.dark_mode
                );
            }
            // The chip's mark is a painted icon now (`theme::icons::Icon::state`),
            // so there is no font glyph left to check for emptiness here.
        }
    }

    #[test]
    fn every_chunk_status_and_log_level_resolves() {
        for palette in [dark_palette(), light_palette()] {
            for status in [
                ChunkStatus::Pending,
                ChunkStatus::Active,
                ChunkStatus::Completed,
                ChunkStatus::Failed,
            ] {
                assert_ne!(palette.chunk_color(status), Color32::TRANSPARENT);
            }
            for level in ["error", "warn", "info", "debug", "trace"] {
                assert_ne!(palette.log_color(level), Color32::TRANSPARENT);
            }
        }
    }

    #[test]
    fn state_colours_are_distinguishable_in_both_themes() {
        // Failed and Interrupted must not share a colour: they are different
        // user actions ("restart" vs "resume").
        for palette in [dark_palette(), light_palette()] {
            assert_ne!(
                palette.state_color(DownloadState::Failed),
                palette.state_color(DownloadState::Interrupted)
            );
            assert_ne!(
                palette.state_color(DownloadState::Running),
                palette.state_color(DownloadState::Merging)
            );
        }
    }

    #[test]
    fn zebra_tint_stays_decorative() {
        // The stripe must be visible but must never compete with the content:
        // the contract deliberately keeps it out of the contrast assertions.
        for palette in [dark_palette(), light_palette()] {
            let ratio = crate::theme::contrast::contrast_ratio(palette.zebra, palette.surface);
            assert!(
                (1.05..1.6).contains(&ratio),
                "dark_mode={} zebra ratio {ratio}",
                palette.dark_mode
            );
        }
    }

    #[test]
    fn layout_modes_follow_the_breakpoints() {
        let b = Breakpoints::default();
        assert_eq!(b.mode(500.0), LayoutMode::Compact);
        assert_eq!(b.mode(900.0), LayoutMode::Regular);
        assert_eq!(b.mode(1440.0), LayoutMode::Wide);
    }
}
