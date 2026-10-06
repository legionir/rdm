//! The floating drop target — a small circle with the rdm mark.
//!
//! A tiny, borderless, always-on-top, transparent window parked in the
//! bottom-right corner of the desktop work area (just above the taskbar clock).
//! Drag a link onto the circle and the *New download* form opens in the main
//! window, pre-filled.
//!
//! ## Why a circle with no text
//!
//! The first version was a 214×104 panel that explained itself in words. On a
//! real desktop that was too much furniture next to the clock, and the user
//! asked for the mark alone. The circle now carries only the rdm glyph; the
//! *explanation* lives where it belongs — Settings, the tray menu and Help.
//!
//! ## Why a drop can still be “empty”
//!
//! Windows hands a dragged *link* to a window as OLE text (or `text/uri-list`);
//! egui's native back end only reports dropped **files** (`CF_HDROP`). A link
//! dragged from a browser therefore often arrives with no path at all — which
//! is why the old target looked like it “accepted nothing”. Two things fix
//! that honestly:
//!
//! 1. anything that *is* readable is used (dropped files, `.url`/`.txt` files
//!    holding a link, `text/uri-list`);
//! 2. if nothing readable arrives, the **clipboard** is tried — a user who just
//!    dragged a link has almost always copied it too — and the app says so in
//!    the status bar instead of pretending the drop worked.
//!
//! A drop is never silent: every outcome produces a sentence (UX policy).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use egui::{Sense, Stroke, ViewportBuilder, ViewportClass, ViewportId};

use crate::clipboard;
use crate::platform::{interpret_drop, DropReport};
use crate::theme::{self, icons::Icon};
use crate::ux;

/// Window size in points: the circle plus a hair of breathing room for the
/// focus ring. The window is transparent, so only the circle is visible.
pub const SIZE: [f32; 2] = [76.0, 76.0];
/// Gap between the circle and the taskbar/desktop edge.
pub const MARGIN: f32 = 10.0;
/// Circle diameter as a fraction of the window (leaves room for the ring).
const DIAMETER: f32 = 0.82;

/// One drop that reached the target, with what the app should say about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDrop {
    /// The link to open the form with, when one was found.
    pub url: Option<String>,
    /// What to write in the status bar — never empty.
    pub note: String,
}

/// Handle shared with the app: the drop target reports back through it.
#[derive(Clone, Default)]
pub struct DropZone {
    /// Drops since the last call.
    drops: Arc<Mutex<Vec<TargetDrop>>>,
    /// A drop happened: the app should reveal itself and open the form.
    activated: Arc<AtomicBool>,
    /// The app's theme, so the circle matches the window.
    dark: Arc<AtomicBool>,
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

    /// Drops since the last call (each one carries its outcome sentence).
    pub fn drain(&self) -> Vec<TargetDrop> {
        self.drops
            .lock()
            .map(|mut drops| std::mem::take(&mut *drops))
            .unwrap_or_default()
    }

    /// Did something land on the target since the last call?
    pub fn take_activation(&self) -> bool {
        self.activated.swap(false, Ordering::Relaxed)
    }

    /// Show (or keep showing) the target. Called every frame while enabled.
    pub fn show(&self, ctx: &egui::Context) {
        let id = ViewportId::from_hash_of("rdm-drop-target");
        let state = self.clone();

        let anchor = crate::windows::bottom_right_anchor(SIZE, MARGIN);
        let mut builder = ViewportBuilder::default()
            .with_title("rdm — drop a link on the mark")
            .with_inner_size(SIZE)
            .with_min_inner_size(SIZE)
            .with_max_inner_size(SIZE)
            .with_resizable(false)
            .with_decorations(false)
            // A desktop helper: no taskbar entry, and the window itself is
            // transparent so only the circle shows.
            .with_taskbar(false)
            .with_transparent(true)
            .with_window_level(egui::WindowLevel::AlwaysOnTop)
            .with_drag_and_drop(true);
        if let Some(position) = anchor {
            builder = builder.with_position(position);
        }

        ctx.show_viewport_deferred(id, builder, move |ctx, class| {
            state.ui(ctx, class, anchor);
        });
    }

    fn ui(&self, ctx: &egui::Context, _class: ViewportClass, anchor: Option<[f32; 2]>) {
        let dark = self.dark.load(Ordering::Relaxed);
        theme::install(ctx, dark);
        let palette = theme::palette_ctx(ctx);

        // Accept what the OS hands us: dropped files, text, or a URI list.
        let (text, uri_list, hovering) = ctx.input(|i| {
            (
                i.raw.dropped_files.first().and_then(|file| {
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
            let mut url = report.url.clone();
            let mut note = report.note.clone();
            if url.is_none() {
                // A link dragged from a browser carries no file path; the
                // clipboard is the honest fallback (and it is announced).
                if let Some(link) = clipboard::url() {
                    note = ux::drop_used_clipboard(&link);
                    url = Some(link);
                }
            }
            if let Ok(mut drops) = self.drops.lock() {
                drops.push(TargetDrop { url, note });
            }
            self.activated.store(true, Ordering::Relaxed);
            // The main window may be hidden in the tray: ask it to run a frame
            // so the drop is picked up immediately.
            ctx.request_repaint_of(ViewportId::ROOT);
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

        // Transparent background: draw nothing but the circle.
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), Sense::hover());
                let center = rect.center();
                let radius = rect.height().min(rect.width()) * DIAMETER * 0.5;

                let (fill, ring) = if hovering {
                    (palette.surface_selected, palette.accent)
                } else if response.hovered() {
                    (palette.surface_hover, palette.accent)
                } else {
                    (palette.surface_window, palette.accent)
                };
                ui.painter()
                    .circle_filled(center, radius, fill);
                let ring_width = if hovering { 3.0 } else { 2.0 };
                ui.painter()
                    .circle_stroke(center, radius, Stroke::new(ring_width, ring));
                // The mark, inset so it never touches the ring.
                let glyph = egui::Rect::from_center_size(center, egui::Vec2::splat(radius * 1.05));
                crate::theme::icons::paint(ui.painter(), Icon::Download, glyph, palette.accent);
                response.on_hover_text("Drop a link here to start a download");
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
    fn the_target_is_a_small_circle_kept_off_the_edges() {
        assert!(SIZE[0] >= 48.0 && SIZE[0] <= 96.0, "small next to the clock");
        assert_eq!(SIZE[0], SIZE[1], "a circle needs a square window");
        assert!(DIAMETER > 0.7 && DIAMETER < 0.95, "the ring needs its margin");
        assert!(MARGIN >= 4.0 && MARGIN <= 32.0);
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

    #[test]
    fn a_drop_is_drained_once_and_activation_is_sticky_until_taken() {
        let zone = DropZone::new();
        assert!(zone.drain().is_empty());
        assert!(!zone.take_activation());
        if let Ok(mut drops) = zone.drops.lock() {
            drops.push(TargetDrop {
                url: Some("https://example.com/x.zip".to_string()),
                note: "Link accepted — the New download form opened with it".to_string(),
            });
        }
        zone.activated.store(true, Ordering::Relaxed);
        let drops = zone.drain();
        assert_eq!(drops.len(), 1);
        assert!(drops[0].url.is_some());
        assert!(!drops[0].note.is_empty(), "every drop has a sentence");
        assert!(zone.drain().is_empty(), "drain takes everything");
        assert!(zone.take_activation(), "activation is sticky until taken");
        assert!(!zone.take_activation());
    }
}
