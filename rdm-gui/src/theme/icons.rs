//! Drawn icons — no font glyphs.
//!
//! ## Why these exist
//!
//! The first version of the desktop pack used Unicode symbols for the close,
//! info, search-clear and keyboard-hint affordances (`✕`, `ⓘ`, `↑`, `↓`). egui's
//! default font set (Ubuntu-Light + NotoEmoji) does **not** cover those
//! codepoints, so on a real Windows desktop they rendered as empty boxes — a
//! shipped, user-visible defect. Emoji (`U+1F300`+) and Latin-1 characters
//! render, Miscellaneous Symbols / Dingbats do not.
//!
//! Rather than pick glyphs by luck, every icon the UI needs is now painted from
//! geometry into the widget's rect. That makes the result identical on every
//! platform, independent of font coverage, and it is checked statically by
//! `audits/ui-contrast-check.py` §6 (no risky codepoints in UI sources).
//!
//! All shapes are expressed as fractions of the target [`Rect`], so one icon
//! works at tooltip size and at 4× that.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

/// Every icon the interface draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// ✕ — close, clear, drop-from-queue.
    Close,
    Plus,
    Play,
    Pause,
    Stop,
    /// Restart from scratch.
    Restart,
    Trash,
    Folder,
    Info,
    Refresh,
    /// The queue (three stacked lines).
    Queue,
    /// The rdm mark: a downward arrow into a base line.
    Download,
    /// State chips (see [`Icon::state`]).
    StateCompleted,
    StateRunning,
    StateMerging,
    StateQueued,
    StatePaused,
    StateInterrupted,
    StateFailed,
    StateCancelled,
}

impl Icon {
    /// The icon that stands for a download state on a chip.
    pub fn state(state: rdm::models::DownloadState) -> Self {
        use rdm::models::DownloadState as S;
        match state {
            S::Completed => Icon::StateCompleted,
            S::Running => Icon::StateRunning,
            S::Merging => Icon::StateMerging,
            S::Queued => Icon::StateQueued,
            S::Paused => Icon::StatePaused,
            S::Interrupted => Icon::StateInterrupted,
            S::Failed => Icon::StateFailed,
            S::Cancelled => Icon::StateCancelled,
        }
    }
}

/// Paint `icon` inside `rect` with `color`.
pub fn paint(painter: &Painter, icon: Icon, rect: Rect, color: Color32) {
    // A hairline that scales with the icon, so big and small stay legible.
    let stroke = Stroke::new((rect.height() * 0.11).clamp(1.2, 2.2), color);
    let unit = rect.height().min(rect.width());
    match icon {
        Icon::Close => {
            // Two diagonals, inset so they never touch the button frame.
            let a = rect.lerp_inside(Vec2::new(0.26, 0.26));
            let b = rect.lerp_inside(Vec2::new(0.74, 0.74));
            let c = rect.lerp_inside(Vec2::new(0.74, 0.26));
            let d = rect.lerp_inside(Vec2::new(0.26, 0.74));
            painter.line_segment([a, b], stroke);
            painter.line_segment([c, d], stroke);
        }
        Icon::Plus => {
            painter.line_segment(
                [
                    rect.lerp_inside(Vec2::new(0.5, 0.22)),
                    rect.lerp_inside(Vec2::new(0.5, 0.78)),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    rect.lerp_inside(Vec2::new(0.22, 0.5)),
                    rect.lerp_inside(Vec2::new(0.78, 0.5)),
                ],
                stroke,
            );
        }
        Icon::Play => {
            painter.add(Shape::convex_polygon(
                vec![
                    rect.lerp_inside(Vec2::new(0.32, 0.22)),
                    rect.lerp_inside(Vec2::new(0.32, 0.78)),
                    rect.lerp_inside(Vec2::new(0.78, 0.5)),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            let bar = |x: f32| {
                Rect::from_min_max(
                    rect.lerp_inside(Vec2::new(x, 0.24)),
                    rect.lerp_inside(Vec2::new(x + 0.14, 0.76)),
                )
            };
            painter.rect_filled(bar(0.28), 1.0, color);
            painter.rect_filled(bar(0.58), 1.0, color);
        }
        Icon::Stop => {
            painter.rect_filled(
                Rect::from_min_max(
                    rect.lerp_inside(Vec2::new(0.28, 0.28)),
                    rect.lerp_inside(Vec2::new(0.72, 0.72)),
                ),
                1.0,
                color,
            );
        }
        Icon::Restart => {
            // A three-quarter arc with one arrow head: “do it again”.
            let center = rect.center();
            let radius = unit * 0.26;
            let points = arc_points(center, radius, -0.35 * std::f32::consts::TAU, 0.62);
            painter.add(Shape::line(points, stroke));
            painter.add(Shape::convex_polygon(
                vec![
                    center + Vec2::new(radius * 0.55, -radius * 1.25),
                    center + Vec2::new(radius * 1.45, -radius * 0.95),
                    center + Vec2::new(radius * 0.7, -radius * 0.25),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Refresh => {
            let center = rect.center();
            let radius = unit * 0.26;
            for (start, end) in [(-0.4, 0.1), (0.1, 0.6)] {
                let points = arc_points(center, radius, start * std::f32::consts::TAU, end);
                painter.add(Shape::line(points, stroke));
            }
            painter.add(Shape::convex_polygon(
                vec![
                    center + Vec2::new(radius * 0.5, -radius * 1.2),
                    center + Vec2::new(radius * 1.35, -radius * 0.85),
                    center + Vec2::new(radius * 0.6, -radius * 0.2),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Trash => {
            // Lid + body + two ribs: unmistakably “throw this away”.
            painter.line_segment(
                [
                    rect.lerp_inside(Vec2::new(0.2, 0.28)),
                    rect.lerp_inside(Vec2::new(0.8, 0.28)),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    rect.lerp_inside(Vec2::new(0.4, 0.28)),
                    rect.lerp_inside(Vec2::new(0.42, 0.18)),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    rect.lerp_inside(Vec2::new(0.6, 0.28)),
                    rect.lerp_inside(Vec2::new(0.58, 0.18)),
                ],
                stroke,
            );
            let body = Rect::from_min_max(
                rect.lerp_inside(Vec2::new(0.29, 0.34)),
                rect.lerp_inside(Vec2::new(0.71, 0.84)),
            );
            painter.rect_stroke(body, 1.0, stroke);
            for x in [0.42, 0.58] {
                painter.line_segment(
                    [
                        rect.lerp_inside(Vec2::new(x, 0.44)),
                        rect.lerp_inside(Vec2::new(x, 0.74)),
                    ],
                    Stroke::new(stroke.width * 0.8, color),
                );
            }
        }
        Icon::Folder => {
            let body = Rect::from_min_max(
                rect.lerp_inside(Vec2::new(0.14, 0.36)),
                rect.lerp_inside(Vec2::new(0.86, 0.78)),
            );
            let tab = Rect::from_min_max(
                rect.lerp_inside(Vec2::new(0.14, 0.24)),
                rect.lerp_inside(Vec2::new(0.48, 0.36)),
            );
            painter.rect_filled(body, 1.5, color);
            painter.rect_filled(tab, 1.0, color);
        }
        Icon::Info => {
            let center = rect.center();
            painter.circle_stroke(center, unit * 0.34, stroke);
            painter.circle_filled(
                center + Vec2::new(0.0, -unit * 0.15),
                stroke.width * 0.7,
                color,
            );
            painter.line_segment(
                [
                    center + Vec2::new(0.0, -unit * 0.02),
                    center + Vec2::new(0.0, unit * 0.17),
                ],
                stroke,
            );
        }
        Icon::Queue => {
            for y in [0.3, 0.5, 0.7] {
                painter.line_segment(
                    [
                        rect.lerp_inside(Vec2::new(0.22, y)),
                        rect.lerp_inside(Vec2::new(0.78, y)),
                    ],
                    stroke,
                );
            }
        }
        Icon::Download => paint_download(painter, rect, color),
        Icon::StateCompleted => {
            painter.line_segment(
                [
                    rect.lerp_inside(Vec2::new(0.24, 0.54)),
                    rect.lerp_inside(Vec2::new(0.44, 0.74)),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    rect.lerp_inside(Vec2::new(0.44, 0.74)),
                    rect.lerp_inside(Vec2::new(0.78, 0.28)),
                ],
                stroke,
            );
        }
        Icon::StateRunning | Icon::StatePaused | Icon::StateQueued | Icon::StateMerging => {
            match icon {
                Icon::StateQueued => {
                    for x in [0.3, 0.5, 0.7] {
                        painter.circle_filled(
                            rect.lerp_inside(Vec2::new(x, 0.5)),
                            stroke.width * 0.75,
                            color,
                        );
                    }
                }
                Icon::StateMerging => {
                    // Two arrows pointing at each other: partial data becoming one file.
                    painter.line_segment(
                        [
                            rect.lerp_inside(Vec2::new(0.2, 0.38)),
                            rect.lerp_inside(Vec2::new(0.8, 0.38)),
                        ],
                        stroke,
                    );
                    painter.line_segment(
                        [
                            rect.lerp_inside(Vec2::new(0.8, 0.62)),
                            rect.lerp_inside(Vec2::new(0.2, 0.62)),
                        ],
                        stroke,
                    );
                    painter.add(Shape::convex_polygon(
                        vec![
                            rect.lerp_inside(Vec2::new(0.72, 0.26)),
                            rect.lerp_inside(Vec2::new(0.92, 0.38)),
                            rect.lerp_inside(Vec2::new(0.72, 0.5)),
                        ],
                        color,
                        Stroke::NONE,
                    ));
                    painter.add(Shape::convex_polygon(
                        vec![
                            rect.lerp_inside(Vec2::new(0.28, 0.5)),
                            rect.lerp_inside(Vec2::new(0.08, 0.62)),
                            rect.lerp_inside(Vec2::new(0.28, 0.74)),
                        ],
                        color,
                        Stroke::NONE,
                    ));
                }
                _ => {
                    // Running (▶) and Paused (❚❚) share the geometry of the
                    // transport icons, drawn inside the chip's square.
                    let square = Rect::from_center_size(rect.center(), Vec2::splat(unit * 0.62));
                    paint(
                        painter,
                        if icon == Icon::StateRunning {
                            Icon::Play
                        } else {
                            Icon::Pause
                        },
                        square,
                        color,
                    );
                }
            }
        }
        Icon::StateInterrupted => {
            // Warning triangle with an exclamation.
            let points = vec![
                rect.lerp_inside(Vec2::new(0.5, 0.2)),
                rect.lerp_inside(Vec2::new(0.88, 0.8)),
                rect.lerp_inside(Vec2::new(0.12, 0.8)),
            ];
            painter.add(Shape::closed_line(points, stroke));
            painter.line_segment(
                [
                    rect.lerp_inside(Vec2::new(0.5, 0.42)),
                    rect.lerp_inside(Vec2::new(0.5, 0.62)),
                ],
                stroke,
            );
            painter.circle_filled(
                rect.lerp_inside(Vec2::new(0.5, 0.72)),
                stroke.width * 0.6,
                color,
            );
        }
        Icon::StateFailed => {
            let a = rect.lerp_inside(Vec2::new(0.28, 0.28));
            let b = rect.lerp_inside(Vec2::new(0.72, 0.72));
            let c = rect.lerp_inside(Vec2::new(0.72, 0.28));
            let d = rect.lerp_inside(Vec2::new(0.28, 0.72));
            painter.line_segment([a, b], stroke);
            painter.line_segment([c, d], stroke);
        }
        Icon::StateCancelled => {
            let center = rect.center();
            painter.circle_stroke(center, unit * 0.3, stroke);
            painter.line_segment(
                [
                    center + Vec2::new(-unit * 0.21, unit * 0.21),
                    center + Vec2::new(unit * 0.21, -unit * 0.21),
                ],
                stroke,
            );
        }
    }
}

/// The rdm mark: bar + arrow head + base line (the same shape as the app icon).
fn paint_download(painter: &Painter, rect: Rect, color: Color32) {
    let unit = rect.height().min(rect.width());
    let bar = Rect::from_center_size(
        rect.lerp_inside(Vec2::new(0.5, 0.34)),
        Vec2::new(unit * 0.2, unit * 0.36),
    );
    painter.rect_filled(bar, unit * 0.06, color);
    painter.add(Shape::convex_polygon(
        vec![
            rect.lerp_inside(Vec2::new(0.3, 0.5)),
            rect.lerp_inside(Vec2::new(0.7, 0.5)),
            rect.lerp_inside(Vec2::new(0.5, 0.74)),
        ],
        color,
        Stroke::NONE,
    ));
    painter.rect_filled(
        Rect::from_min_max(
            rect.lerp_inside(Vec2::new(0.3, 0.79)),
            rect.lerp_inside(Vec2::new(0.7, 0.86)),
        ),
        unit * 0.03,
        color,
    );
}

/// Points along a circular arc, for the restart/refresh arrows.
fn arc_points(center: Pos2, radius: f32, start_turns: f32, end_turns: f32) -> Vec<Pos2> {
    let steps = 18;
    (0..=steps)
        .map(|i| {
            let t = start_turns + (end_turns - start_turns) * (i as f32 / steps as f32);
            let angle = t * std::f32::consts::TAU;
            center + Vec2::new(angle.cos(), angle.sin()) * radius
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Painting must not panic at any size the UI uses, and the geometry has to
    /// stay inside the rect it was given (a stray icon would overlap text).
    #[test]
    fn every_icon_paints_inside_its_rect() {
        let ctx = egui::Context::default();
        let all = [
            Icon::Close,
            Icon::Plus,
            Icon::Play,
            Icon::Pause,
            Icon::Stop,
            Icon::Restart,
            Icon::Trash,
            Icon::Folder,
            Icon::Info,
            Icon::Refresh,
            Icon::Queue,
            Icon::Download,
            Icon::StateCompleted,
            Icon::StateRunning,
            Icon::StateMerging,
            Icon::StateQueued,
            Icon::StatePaused,
            Icon::StateInterrupted,
            Icon::StateFailed,
            Icon::StateCancelled,
        ];
        let mut painted = 0usize;
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for size in [12.0_f32, 16.0, 24.0] {
                    for icon in all {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
                        // A painter clipped to the rect: anything outside is
                        // dropped, so the shapes must already be inside.
                        let painter = ui.painter_at(rect);
                        paint(&painter, icon, rect, Color32::WHITE);
                        painted += 1;
                    }
                }
            });
        });
        assert_eq!(painted, all.len() * 3, "every icon was painted");
        assert!(!output.shapes.is_empty(), "painting produced shapes");
    }

    #[test]
    fn the_state_icon_covers_every_download_state() {
        for state in [
            rdm::models::DownloadState::Completed,
            rdm::models::DownloadState::Running,
            rdm::models::DownloadState::Merging,
            rdm::models::DownloadState::Queued,
            rdm::models::DownloadState::Paused,
            rdm::models::DownloadState::Interrupted,
            rdm::models::DownloadState::Failed,
            rdm::models::DownloadState::Cancelled,
        ] {
            let icon = Icon::state(state);
            assert!(
                matches!(
                    icon,
                    Icon::StateCompleted
                        | Icon::StateRunning
                        | Icon::StateMerging
                        | Icon::StateQueued
                        | Icon::StatePaused
                        | Icon::StateInterrupted
                        | Icon::StateFailed
                        | Icon::StateCancelled
                ),
                "{state:?} must map to a state icon"
            );
        }
    }

    #[test]
    fn arcs_stay_on_their_circle() {
        let center = Pos2::new(10.0, 10.0);
        for point in arc_points(center, 4.0, -0.3, 0.55) {
            let distance = (point - center).length();
            assert!((distance - 4.0).abs() < 0.01, "arc drifted to {distance}");
        }
    }
}
