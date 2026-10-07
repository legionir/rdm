//! The floating drop target — a small circle with the rdm mark on it.
//!
//! A tiny, borderless, always-on-top, transparent window parked in the
//! bottom-right corner of the desktop work area (just above the taskbar clock).
//! Drag a link onto the circle and the *New download* form opens in the main
//! window, pre-filled.
//!
//! ## Why a circle with the app mark and no text (round 6)
//!
//! The first version was a 214×104 panel that explained itself in words; the
//! round-5 version was a 76 pt circle with the generic drawn *download* glyph.
//! On a real desktop both were still too much furniture next to the clock, so
//! the mark is now **52 pt** and carries **the application icon itself** — the
//! same 64×64 RGBA buffer as the window, taskbar and tray icon. Nothing is
//! written on it: not a label, not a tooltip. What it is, is the app's own face;
//! the explanation lives in Settings, the tray menu and Help.
//!
//! ## Why a drop can still be “empty”
//!
//! A drag carries whatever its source put on the clipboard, and the sources do
//! not agree on how to describe a link:
//!
//! * a browser offers **`CF_UNICODETEXT`** (and `text/uri-list`) — the URL as
//!   text, *not* a file;
//! * a file manager offers **`CF_HDROP`** (the paths), and a dropped `.url`
//!   file holds the link inside itself;
//! * a Qt/GTK application offers **`text/uri-list`**.
//!
//! On Windows the window's own drag-and-drop only accepts `CF_HDROP` and
//! refuses the rest *before the app is told anything*, which is why dropping a
//! link used to do nothing at all — see [`crate::oledrop`], which registers the
//! mark's own target for the three formats above. Elsewhere the window manager
//! hands the URI list to the toolkit, and egui's input is used directly.
//!
//! Whichever path a drop arrives on, this module turns it into one sentence and
//! at most one link (see [`resolve_drop`]) — and if nothing readable arrives,
//! the **clipboard** is tried, because a user who just dragged a link has
//! almost always copied it too. A drop is never silent.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};

use egui::{Sense, Stroke, ViewportBuilder, ViewportClass, ViewportId};

use crate::clipboard;
use crate::icon;
use crate::platform::interpret_drop;
use crate::theme;
use crate::ux;

/// Window size in points: the circle plus a hair of breathing room for the
/// focus ring. The window is transparent, so only the circle is visible.
///
/// 52 pt is small enough to sit beside the clock without being furniture, and
/// large enough to hit with a dragged link (the circle fills 88% of it).
pub const SIZE: [f32; 2] = [52.0, 52.0];
/// Gap between the circle and the taskbar/desktop edge.
pub const MARGIN: f32 = 10.0;
/// Circle diameter as a fraction of the window (leaves room for the ring).
const DIAMETER: f32 = 0.88;
/// Side of the app mark, as a multiple of the circle's radius.
const LOGO_SCALE: f32 = 1.26;
/// The mark is the 64×64 asset box-filtered to this grid (one coloured quad per
/// cell). 32 keeps the glyph crisp at ~29 pt and costs 4096 vertices a frame.
const LOGO_GRID: usize = 32;

/// What a finished drop carried, in the order the drop sources offer it.
///
/// Deliberately dumb: no interpretation happens here, so every “what does this
/// mean” decision stays in [`resolve_drop`] with its tests.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DropPayload {
    /// Real files (`CF_HDROP`); a dropped `.url`/`.txt` holding a link counts.
    pub files: Vec<PathBuf>,
    /// Plain text — where a browser puts the link it is dragging.
    pub text: Option<String>,
    /// `text/uri-list` — what Qt/GTK applications put their URIs in.
    pub uri_list: Option<String>,
}

impl DropPayload {
    /// Did anything readable survive in here?
    ///
    /// The Windows target deliberately accepts a little more than this (see
    /// [`crate::oledrop`]): a drag it cannot read is still a drag the user
    /// made, and refusing it would leave them with a mark that ignores them.
    /// What arrives without a readable link is announced, not swallowed.
    pub fn is_readable(&self) -> bool {
        !self.files.is_empty() || self.text.is_some() || self.uri_list.is_some()
    }
}

/// One drop that reached the target, with what the app should say about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDrop {
    /// The link to open the form with, when one was found.
    pub url: Option<String>,
    /// What to write in the status bar — never empty.
    pub note: String,
}

/// The drop policy, as a pure function: payload in, sentence and link out.
///
/// `clipboard_link` is the last resort, and passing it in (instead of reading
/// the clipboard here) is what makes every branch of this testable.
pub fn resolve_drop(payload: &DropPayload, clipboard_link: Option<String>) -> TargetDrop {
    // The same precedence the app has always used: text first (a browser's
    // link), then the URI list, then paths (a `.url` file holds its link).
    let first_file = payload
        .files
        .first()
        .map(|path| path.display().to_string());
    let from_files = uri_list_from_files(&payload.files);
    let report = interpret_drop(
        payload.text.as_deref().or(first_file.as_deref()),
        payload.uri_list.as_deref().or(from_files.as_deref()),
    );

    if let Some(url) = report.url {
        return TargetDrop {
            url: Some(url),
            note: report.note,
        };
    }
    // A `text/uri-list` that points at links rather than at files: the shared
    // interpreter only looks inside `file://` entries, so a drag from a Qt/GTK
    // application that spells the link out in the list is read here.
    if let Some(list) = payload.uri_list.as_deref().or(from_files.as_deref()) {
        if let Some(url) = crate::platform::first_url_in(list) {
            return TargetDrop {
                url: Some(url.clone()),
                note: format!("Dropped link accepted: {url}"),
            };
        }
    }
    match clipboard_link {
        // A link dragged from a browser carries no file path on some sources;
        // the clipboard is the honest fallback, and it is announced.
        Some(link) => TargetDrop {
            url: Some(link.clone()),
            note: ux::drop_used_clipboard(&link),
        },
        None => TargetDrop {
            url: None,
            note: report.note,
        },
    }
}

/// `file://` URIs for the dropped paths, the shape [`interpret_drop`] reads.
fn uri_list_from_files(files: &[PathBuf]) -> Option<String> {
    if files.is_empty() {
        return None;
    }
    Some(
        files
            .iter()
            .map(|path| file_uri(path))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// One path as a `file://` URI (`file:///home/x`, `file:///C:/x` — the second
/// slash counts on Windows, and `parse_uri_list` strips the leading one).
fn file_uri(path: &std::path::Path) -> String {
    let slashed = path.display().to_string().replace('\\', "/");
    if slashed.starts_with('/') {
        format!("file://{slashed}")
    } else {
        format!("file:///{slashed}")
    }
}

/// The app mark as a mesh: the embedded 64×64 icon, box-filtered down to
/// `grid`×`grid` coloured quads laid out inside `rect`.
///
/// A mesh rather than a texture on purpose: a vertex carries its own colour, so
/// the mark needs no texture upload, no renderer support beyond triangles, and
/// the part that can be checked without a screen — the downsampling and the
/// geometry — is an ordinary unit test.
fn logo_mesh(rect: egui::Rect, grid: usize) -> egui::Mesh {
    let grid = grid.clamp(1, icon::ICON_SIZE as usize);
    // The source pixel a cell starts at; whole grid, whole asset, whatever the
    // grid divides by.
    let source = |cell: usize| cell * icon::ICON_SIZE as usize / grid;
    let mut mesh = egui::Mesh::default();

    for row in 0..grid {
        for column in 0..grid {
            let mut alpha_sum: u32 = 0;
            let mut red: u32 = 0;
            let mut green: u32 = 0;
            let mut blue: u32 = 0;
            for y in source(row)..source(row + 1) {
                for x in source(column)..source(column + 1) {
                    let px = (y * icon::ICON_SIZE as usize + x) * 4;
                    let a = icon::ICON_RGBA[px + 3] as u32;
                    // Weight the colour by its alpha (premultiplied averaging):
                    // the transparent corners of the asset must not darken the
                    // cells they only partly cover.
                    alpha_sum += a;
                    red += icon::ICON_RGBA[px] as u32 * a / 255;
                    green += icon::ICON_RGBA[px + 1] as u32 * a / 255;
                    blue += icon::ICON_RGBA[px + 2] as u32 * a / 255;
                }
            }
            let rows = source(row + 1) - source(row);
            let columns = source(column + 1) - source(column);
            let cells = (rows * columns) as u32;
            let color = egui::Color32::from_rgba_premultiplied(
                (red / cells) as u8,
                (green / cells) as u8,
                (blue / cells) as u8,
                (alpha_sum / cells) as u8,
            );

            let x0 = rect.left() + rect.width() * column as f32 / grid as f32;
            let y0 = rect.top() + rect.height() * row as f32 / grid as f32;
            let x1 = rect.left() + rect.width() * (column + 1) as f32 / grid as f32;
            let y1 = rect.top() + rect.height() * (row + 1) as f32 / grid as f32;
            // A third of a point of overlap, so neighbouring quads cannot let
            // the circle behind them show through a seam.
            let cell =
                egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1)).expand(0.35);
            mesh.add_colored_rect(cell, color);
        }
    }
    mesh
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
    /// A drag that carries something readable is over the mark right now.
    ///
    /// Set by the OLE target's `DragEnter`/`DragLeave` on Windows and by the
    /// toolkit's hovered-files input elsewhere, so the ring lights up either
    /// way without the two paths knowing about each other.
    hover: Arc<AtomicBool>,
    /// Where to ask for the next frame from a drop that arrives outside one.
    ctx: Arc<Mutex<Option<egui::Context>>>,
    /// The mark's window handle, once resolved (0 = not yet).
    target_hwnd: Arc<AtomicIsize>,
    /// The OS refused the drop target: do not ask again this session.
    target_refused: Arc<AtomicBool>,
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

    /// A drag released over the mark: decide what it means and tell the app.
    ///
    /// Every arrival path ends here — the OLE target on Windows, egui's input
    /// elsewhere — so the status sentence and the form's pre-fill cannot differ
    /// between platforms.
    pub fn accept(&self, payload: DropPayload) {
        let drop = resolve_drop(&payload, clipboard::url());
        tracing::info!(
            "drop target: {} file(s), text {}, uri-list {} -> {}{}",
            payload.files.len(),
            payload.text.is_some(),
            payload.uri_list.is_some(),
            if drop.url.is_some() { "a link" } else { "no link" },
            if drop.url.is_some() {
                ""
            } else {
                " (the sentence says why)"
            }
        );
        if let Ok(mut drops) = self.drops.lock() {
            drops.push(drop);
        }
        self.activated.store(true, Ordering::Relaxed);
        self.wake_root();
    }

    /// Show (or keep showing) the target. Called every frame while enabled.
    pub fn show(&self, ctx: &egui::Context) {
        let id = ViewportId::from_hash_of("rdm-drop-target");
        let state = self.clone();
        // Kept so a drop that arrives between frames can still wake the app.
        if let Ok(mut slot) = self.ctx.lock() {
            *slot = Some(ctx.clone());
        }

        let anchor = crate::windows::bottom_right_anchor(SIZE, MARGIN);
        let mut builder = ViewportBuilder::default()
            .with_title(crate::windows::DROP_TARGET_TITLE)
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
            // Kept on for the platforms where the toolkit's own drop path is
            // the only one; on Windows this is the target `oledrop` takes over.
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

        #[cfg(target_os = "windows")]
        self.ensure_target(ctx);
        #[cfg(not(target_os = "windows"))]
        self.read_input_drops(ctx);

        let dragging = self.hover.load(Ordering::Relaxed);

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

                let (fill, ring) = if dragging {
                    (palette.surface_selected, palette.accent)
                } else if response.hovered() {
                    (palette.surface_hover, palette.accent)
                } else {
                    (palette.surface_window, palette.accent)
                };
                let painter = ui.painter();
                painter.circle_filled(center, radius, fill);
                let ring_width = if dragging { 3.0 } else { 2.0 };
                painter.circle_stroke(center, radius, Stroke::new(ring_width, ring));
                // The app mark, and nothing else: no label, no tooltip. Its
                // transparent corners let the circle's own fill show through.
                let logo =
                    egui::Rect::from_center_size(center, egui::Vec2::splat(radius * LOGO_SCALE));
                painter.add(egui::Shape::Mesh(logo_mesh(logo, LOGO_GRID)));

                // Deliberately no tooltip and no label: the mark shows the
                // app's own face and explains nothing (round 6). The only
                // thing worth saying is said in the status bar, after a drop.
            });
    }

    /// Take over the mark's window as an OLE drop target (Windows).
    ///
    /// Runs on the UI thread, once, after the window exists — the handle is
    /// found by title (see [`crate::windows::drop_target_window`]) because
    /// eframe does not expose a viewport's native handle.
    #[cfg(target_os = "windows")]
    fn ensure_target(&self, ctx: &egui::Context) {
        if self.target_refused.load(Ordering::Relaxed) {
            return;
        }
        let known = self.target_hwnd.load(Ordering::Relaxed);
        let hwnd = if known != 0 && crate::windows::is_window(known) {
            return; // already installed on this very window
        } else {
            match crate::windows::drop_target_window() {
                Some(hwnd) => hwnd,
                None => return, // the window is not created yet
            }
        };

        let hover = Arc::clone(&self.hover);
        let wake = ctx.clone();
        let zone = self.clone();
        let sink = crate::oledrop::Sink {
            hover: Box::new(move |over| {
                // The circle lights up on the very frame the drag arrives, even
                // though the app is not painting: ask for a repaint from here.
                hover.store(over, Ordering::Relaxed);
                wake.request_repaint();
            }),
            drop: Box::new(move |payload| zone.accept(payload)),
        };

        match crate::oledrop::install(hwnd, sink) {
            Ok(()) => {
                self.target_hwnd.store(hwnd, Ordering::Relaxed);
            }
            Err(err) => {
                self.target_refused.store(true, Ordering::Relaxed);
                self.hover.store(false, Ordering::Relaxed);
                tracing::warn!(
                    "drop target: {err} — the mark keeps the window's own (files-only) drops"
                );
            }
        }
    }

    /// egui's own drop input: what the window managers elsewhere hand the
    /// toolkit. A browser puts a dragged link in the plain text / URI list, so
    /// this is the same payload the Windows target builds, from the same fields.
    #[cfg(not(target_os = "windows"))]
    fn read_input_drops(&self, ctx: &egui::Context) {
        let (payload, hovered) = ctx.input(|i| {
            let hovered = !i.raw.hovered_files.is_empty();
            let files: Vec<PathBuf> = i
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect();
            let text = i.raw.dropped_files.first().and_then(|file| {
                file.path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .or_else(|| (!file.name.is_empty()).then(|| file.name.clone()))
            });
            (
                (!i.raw.dropped_files.is_empty()).then(|| DropPayload {
                    uri_list: uri_list_from_files(&files),
                    files,
                    text,
                }),
                hovered,
            )
        });
        self.hover.store(hovered, Ordering::Relaxed);
        if let Some(payload) = payload {
            self.accept(payload);
        }
    }

    /// Ask the main window for a frame, from wherever the drop was noticed.
    ///
    /// A COM callback is not inside a frame and the main window may be hidden in
    /// the tray, so this is what turns “something landed on the mark” into the
    /// app actually running: the next frame sees [`Self::take_activation`] and
    /// reveals the window itself (`app::sync_drop_target`).
    fn wake_root(&self) {
        let Ok(slot) = self.ctx.lock() else {
            return;
        };
        let Some(ctx) = slot.as_ref() else {
            return; // no frame has run yet: the first one will see the flag
        };
        ctx.request_repaint_of(ViewportId::ROOT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_is_a_small_circle_kept_off_the_edges() {
        // “A small circle by the clock”, as reported: smaller than the 76 pt
        // round-5 mark, still comfortably draggable.
        assert!(SIZE[0] >= 40.0 && SIZE[0] <= 64.0, "small next to the clock");
        assert_eq!(SIZE[0], SIZE[1], "a circle needs a square window");
        assert!(DIAMETER > 0.7 && DIAMETER < 0.95, "the ring needs its margin");
        assert!(MARGIN >= 4.0 && MARGIN <= 32.0);
        // The mark is the biggest thing on the circle, and it still stays
        // inside the ring: half a mark is at most 85% of the radius.
        assert!(LOGO_SCALE > 1.0, "the mark should fill the circle");
        assert!(
            LOGO_SCALE * 0.5 < 0.85,
            "the mark would grow over the ring at LOGO_SCALE {LOGO_SCALE}"
        );
    }

    #[test]
    fn the_logo_is_the_app_icon_and_nothing_else() {
        let rect = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::Vec2::splat(29.0));
        let mesh = logo_mesh(rect, LOGO_GRID);

        // One quad per cell: four vertices and six indices each.
        assert_eq!(mesh.vertices.len(), LOGO_GRID * LOGO_GRID * 4);
        assert_eq!(mesh.indices.len(), LOGO_GRID * LOGO_GRID * 6);
        // Everything stays inside the box it was given (bar the seam overlap).
        for vertex in &mesh.vertices {
            assert!(
                rect.expand(0.5).contains(vertex.pos),
                "a logo vertex escaped its rect: {:?}",
                vertex.pos
            );
        }
        // The asset's rounded corners are transparent, so the corners of the
        // mesh must be too — otherwise the mark would be a square on the
        // circle instead of the app's own rounded mark.
        let corner = mesh.vertices[0].color;
        assert_eq!(corner.a(), 0, "the mark's corner is not transparent");
    }

    #[test]
    fn the_logo_keeps_the_icons_colours() {
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::Vec2::splat(32.0));
        let mesh = logo_mesh(rect, 16);
        // The asset is a blue rounded square with a white glyph on it: both
        // colours have to survive the box filter.
        let is_blue = |c: &egui::Color32| c.b() > 120 && c.b() > c.r() + 40;
        let is_light = |c: &egui::Color32| c.r() > 180 && c.g() > 180 && c.a() > 180;
        assert!(
            mesh.vertices.iter().any(|v| is_blue(&v.color)),
            "the mark lost its blue"
        );
        assert!(
            mesh.vertices.iter().any(|v| is_light(&v.color)),
            "the mark lost the light glyph"
        );
        // ... and nothing is a colour the asset never contains (a red channel
        // past the blue one would mean the channels got mixed up).
        assert!(mesh.vertices.iter().all(|v| v.color.r() <= v.color.b() + 40));
    }

    #[test]
    fn a_dropped_link_is_taken_from_the_text() {
        // What a browser drag looks like: the URL in the plain text.
        let drop = resolve_drop(
            &DropPayload {
                text: Some("https://example.com/big.iso".to_string()),
                ..Default::default()
            },
            None,
        );
        assert_eq!(drop.url.as_deref(), Some("https://example.com/big.iso"));
        assert!(!drop.note.is_empty());

        // ... and what a Qt/GTK drag looks like: the link spelled out in the
        // URI list, with no file involved anywhere.
        let drop = resolve_drop(
            &DropPayload {
                uri_list: Some("https://example.com/from-list.zip".to_string()),
                ..Default::default()
            },
            None,
        );
        assert_eq!(
            drop.url.as_deref(),
            Some("https://example.com/from-list.zip")
        );
    }

    #[test]
    fn a_dropped_url_file_gives_up_the_link_inside_it() {
        // A `.url` file dragged in from the desktop: the link is inside the
        // file, and that is a drop the app has always accepted.
        let path = std::env::temp_dir().join("rdm-dropzone-test.url");
        std::fs::write(&path, "[InternetShortcut]\nURL=https://example.com/in-file.iso\n")
            .expect("write the fixture");
        let drop = resolve_drop(
            &DropPayload {
                files: vec![path.clone()],
                ..Default::default()
            },
            None,
        );
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            drop.url.as_deref(),
            Some("https://example.com/in-file.iso"),
            "a dropped shortcut must still hand over its link"
        );
    }

    #[test]
    fn a_dropped_file_without_a_link_says_so() {
        let path = std::env::temp_dir().join("rdm-dropzone-test.bin");
        std::fs::write(&path, "not a link at all").expect("write the fixture");
        let drop = resolve_drop(
            &DropPayload {
                files: vec![path.clone()],
                ..Default::default()
            },
            // Not even the clipboard may talk the app into “it worked”: the
            // payload answered, and the answer was no.
            Some("https://example.com/from-clipboard.zip".to_string()),
        );
        let _ = std::fs::remove_file(&path);
        assert_eq!(drop.url, None);
        assert!(
            drop.note.contains("does not contain a link"),
            "the sentence has to explain: {:?}",
            drop.note
        );
    }

    #[test]
    fn the_clipboard_is_the_last_resort_and_it_is_announced() {
        let link = "https://example.com/from-clipboard.zip".to_string();
        let drop = resolve_drop(&DropPayload::default(), Some(link.clone()));
        assert_eq!(drop.url.as_deref(), Some(link.as_str()));
        assert!(
            drop.note.contains(&link),
            "the sentence has to name the link it used: {:?}",
            drop.note
        );

        // Payload that yields no link at all: the clipboard is still the last
        // resort — that is the behaviour the app has always had — and the
        // sentence names the link so the user can tell where it came from.
        let drop = resolve_drop(
            &DropPayload {
                text: Some("just some words".to_string()),
                ..Default::default()
            },
            Some(link.clone()),
        );
        assert_eq!(drop.url.as_deref(), Some(link.as_str()));
        assert!(drop.note.contains(&link));

        // ... and with an empty clipboard the drop still speaks: the sentence
        // explains that nothing readable arrived.
        let drop = resolve_drop(
            &DropPayload {
                text: Some("just some words".to_string()),
                ..Default::default()
            },
            None,
        );
        assert_eq!(drop.url, None);
        assert!(!drop.note.is_empty(), "a drop is never silent");
        assert!(drop.note.contains("looks like a link"), "{:?}", drop.note);
    }

    #[test]
    fn a_drop_is_drained_once_and_activation_is_sticky_until_taken() {
        let zone = DropZone::new();
        assert!(zone.drain().is_empty());
        assert!(!zone.take_activation());
        zone.accept(DropPayload {
            text: Some("https://example.com/x.zip".to_string()),
            ..Default::default()
        });
        let drops = zone.drain();
        assert_eq!(drops.len(), 1);
        assert!(drops[0].url.is_some());
        assert!(!drops[0].note.is_empty(), "every drop has a sentence");
        assert!(zone.drain().is_empty(), "drain takes everything");
        assert!(zone.take_activation(), "activation is sticky until taken");
        assert!(!zone.take_activation());
    }
}
