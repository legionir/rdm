//! The floating drop target (“floating bottom”).
//!
//! A small, borderless, always-on-top window anchored to the bottom-right of
//! the desktop work area — i.e. just above the clock area of the taskbar. Drag
//! a link from a browser onto it and the *New download* dialog opens in the
//! main window, pre-filled with it.
//!
//! It is its own egui viewport (`show_viewport_deferred`), so it stays visible
//! while the main window is hidden in the tray. The window is intentionally
//! tiny so it cannot get in the way; the ✕ button hides it and turns the
//! setting off (one click, no dialog — nothing is destroyed).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use egui::{Context, RichText, ViewportBuilder, ViewportClass, ViewportId};

use crate::platform::{interpret_drop, DropReport, ImportBus};
use crate::theme::{self, components, Spacing};

/// Window size in points. Compact enough to park next to the clock.
pub const SIZE: [f32; 2] = [214.0, 104.0];
/// Gap between the target and the taskbar/desktop edge.
pub const MARGIN: f32 = 10.0;

/// Handle shared with the app: the drop target reports back through it.
#[derive(Clone, Default)]
pub struct DropZone {
    bus: ImportBus,
    hide_requested: Arc<AtomicBool>,
    /// A drop happened: the app should reveal itself and open the form.
    activated: Arc<AtomicBool>,
    /// The app's theme, so the target matches the window.
    dark: Arc<AtomicBool>,
    /// The last outcome, shown inside the target so a drop is never silent.
    last: Arc<std::sync::Mutex<Option<String>>>,
}

impl DropZone {
    pub fn new() -> Self {
        let zone = Self::default();
        zone.dark.store(true, Ordering::Relaxed);
        zone
    }

    /// Follow the app's light/dark choice.
    pub fn set_dark(&self, dark: bool) {
        self.dark.store(dark, Ordering::Relaxed);
    }

    /// Links dropped on the target since the last call.
    pub fn drain(&self) -> Vec<String> {
        self.bus.drain()
    }

    /// Did the user press ✕ on the target?
    pub fn take_hide_request(&self) -> bool {
        self.hide_requested.swap(false, Ordering::Relaxed)
    }

    /// Did something land on the target since the last call?
    pub fn take_activation(&self) -> bool {
        self.activated.swap(false, Ordering::Relaxed)
    }

    /// Hide the target programmatically (the ✕ button inside it).
    pub fn request_hide(&self) {
        self.hide_requested.store(true, Ordering::Relaxed);
    }

    fn note(&self, text: impl Into<String>) {
        if let Ok(mut last) = self.last.lock() {
            *last = Some(text.into());
        }
    }

    fn last(&self) -> Option<String> {
        self.last.lock().ok().and_then(|last| last.clone())
    }

    /// Show (or keep showing) the target. Called every frame while enabled.
    pub fn show(&self, ctx: &Context) {
        let id = ViewportId::from_hash_of("rdm-drop-target");
        let state = self.clone();

        let anchor = crate::windows::bottom_right_anchor(SIZE, MARGIN);
        let mut builder = ViewportBuilder::default()
            .with_title("rdm — drop a link here")
            .with_inner_size(SIZE)
            .with_min_inner_size(SIZE)
            .with_max_inner_size(SIZE)
            .with_resizable(false)
            .with_decorations(false)
            // A desktop helper, not another taskbar entry.
            .with_taskbar(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop)
            .with_drag_and_drop(true);
        if let Some(position) = anchor {
            builder = builder.with_position(position);
        }

        ctx.show_viewport_deferred(id, builder, move |ctx, class| {
            state.ui(ctx, class, anchor);
        });
    }

    fn ui(&self, ctx: &Context, _class: ViewportClass, anchor: Option<[f32; 2]>) {
        let (dark, last) = (self.dark.load(Ordering::Relaxed), self.last());
        theme::install(ctx, dark);
        let palette = theme::palette_ctx(ctx);
        let spacing = Spacing::default();

        // Accept what the OS hands us: text (browsers) or file URI lists.
        let (text, uri_list, hovering) = ctx.input(|i| {
            (
                i.raw
                    .dropped_files
                    .first()
                    .and_then(|file| {
                        file.path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .or_else(|| (!file.name.is_empty()).then(|| file.name.clone()))
                    }),
                dropped_uri_list(&i.raw.dropped_files),
                !i.raw.hovered_files.is_empty(),
            )
        });

        if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
            let report: DropReport = interpret_drop(text.as_deref(), uri_list.as_deref());
            if let Some(url) = report.url.clone() {
                self.bus.push(url);
                // The main window may be hidden in the tray: ask it to wake up
                // so the import is picked up in the same instant.
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }
            self.note(report.note.clone());
            self.activated.store(true, Ordering::Relaxed);
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Informational,
            ));
        }

        if let Some(position) = anchor {
            // Keep the target anchored even after a DPI or taskbar change.
            let current = ctx.input(|i| i.viewport().outer_rect.map(|r| r.min));
            if current.map(|c| [c.x, c.y]) != Some(position) {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                    position[0], position[1],
                )));
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(palette.surface_window).rounding(6.0_f32)
                .stroke(egui::Stroke::new(1.0_f32, palette.accent)))
            .show(ctx, |ui| {
                ui.add_space(spacing.xs);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⬇ Drop a link here").strong().color(palette.text_primary));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("✕")
                            .on_hover_text("Hide the drop target (the setting turns off)")
                            .clicked()
                        {
                            self.request_hide();
                        }
                    });
                });
                let hint = if hovering {
                    "Release to start a download"
                } else {
                    "Drag a link from your browser onto this box"
                };
                components::hint(ui, &palette, hint);
                ui.add_space(spacing.xs);
                match last {
                    Some(note) => {
                        ui.label(RichText::new(note).small().color(palette.text_muted));
                    }
                    None => {
                        ui.label(
                            RichText::new("The new-download form opens with the link filled in.")
                                .small()
                                .color(palette.text_muted),
                        );
                    }
                }
            });
    }
}

/// egui exposes dropped files as paths; browsers may also deliver the link as
/// a file name. Anything else has to be unwrapped by the caller.
fn dropped_uri_list(dropped: &[egui::DroppedFile]) -> Option<String> {
    let paths: Vec<String> = dropped
        .iter()
        .filter_map(|file| file.path.as_ref())
        .map(|path| format!("file://{}", path.display()))
        .collect();
    if paths.is_empty() {
        None
    } else {
        Some(paths.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_is_small_and_kept_off_the_edges() {
        assert!(SIZE[0] >= 160.0 && SIZE[0] <= 320.0);
        assert!(SIZE[1] >= 72.0 && SIZE[1] <= 160.0);
        assert!(MARGIN >= 4.0 && MARGIN <= 32.0);
    }

    #[test]
    fn a_queued_drop_is_drained_once_and_requests_are_sticky_until_taken() {
        let zone = DropZone::new();
        assert!(zone.drain().is_empty());
        assert!(!zone.take_hide_request());
        assert!(!zone.take_activation());
        zone.note("Dropped link accepted: https://example.com/x.zip");
        assert!(zone.last().unwrap().contains("accepted"));
        zone.request_hide();
        assert!(zone.take_hide_request(), "✕ is a one-shot request");
        assert!(!zone.take_hide_request());
    }

    #[test]
    fn a_dropped_path_becomes_a_uri_list() {
        let dropped = vec![egui::DroppedFile {
            path: Some(std::path::PathBuf::from("/tmp/a b.txt")),
            ..Default::default()
        }];
        let list = dropped_uri_list(&dropped).unwrap();
        assert!(list.starts_with("file://"));
        assert!(list.contains("a b.txt"));
    }
}
