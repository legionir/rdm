//! Terminal progress rendering (indicatif).

use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::utils::human::{self, Eta};

/// Throttled orchestration of the aggregate + per-chunk progress bars.
pub struct ProgressUi {
    enabled: bool,
    multi: MultiProgress,
    total: ProgressBar,
    chunk_bars: Vec<Option<ProgressBar>>,
}

impl ProgressUi {
    pub fn new(filename: &str, total: u64, connections: usize, no_progress: bool) -> Self {
        if no_progress {
            return ProgressUi {
                enabled: false,
                multi: MultiProgress::new(),
                total: ProgressBar::hidden(),
                chunk_bars: Vec::new(),
            };
        }
        let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stderr());
        let total_bar = multi.add(ProgressBar::new(total));
        total_bar.set_style(
            ProgressStyle::with_template(
                "{msg} {wide_bar} {percent:>3}% {bytes}/{total_bytes} {bytes_per_sec} ETA {eta}",
            )
            .unwrap()
            .progress_chars("##-"),
        );
        total_bar.set_message(filename.to_string());
        total_bar.enable_steady_tick(Duration::from_millis(250));

        let mut chunk_bars = Vec::new();
        if connections <= 8 {
            for i in 0..connections {
                let bar = multi.add(ProgressBar::new(0));
                bar.set_style(
                    ProgressStyle::with_template(
                        "[c{idx:02}] {wide_bar} {bytes}/{total_bytes} {bytes_per_sec} {msg}",
                    )
                    .unwrap()
                    .progress_chars("=>-"),
                );
                let _ = i;
                chunk_bars.push(Some(bar));
            }
        }

        ProgressUi {
            enabled: true,
            total: total_bar,
            chunk_bars,
            multi,
        }
    }

    pub fn set_total(&mut self, downloaded: u64) {
        if !self.enabled {
            return;
        }
        self.total.set_position(downloaded);
    }

    pub fn set_connections(&mut self, active: usize, completed: usize, total_chunks: usize) {
        if !self.enabled {
            return;
        }
        let msg = format!("{active} active · {completed}/{total_chunks} chunks");
        self.total.set_message(msg);
    }

    pub fn set_eta(&mut self, remaining: u64, elapsed: Duration) {
        if !self.enabled || remaining == 0 {
            return;
        }
        let secs = elapsed.as_secs().max(1) as f64;
        let rate = /* bytes per second so far */ {
            let pos = self.total.position();
            pos as f64 / secs
        };
        if rate > 0.0 {
            let eta = (remaining as f64 / rate).ceil() as u64;
            let _ = rate;
            let _ = Eta(Some(eta));
        }
    }

    pub fn set_phase(&mut self, phase: &str) {
        if !self.enabled {
            return;
        }
        self.total.set_message(format!("{phase} —"));
    }

    pub fn set_chunk_progress(&mut self, idx: usize, bytes: u64) {
        if !self.enabled {
            return;
        }
        if let Some(Some(bar)) = self.chunk_bars.get_mut(idx) {
            bar.set_position(bytes);
        }
    }

    pub fn set_chunk_message(&mut self, idx: usize, msg: String) {
        if !self.enabled {
            return;
        }
        if let Some(Some(bar)) = self.chunk_bars.get_mut(idx) {
            bar.set_message(msg);
        }
    }

    pub fn finish_chunk(&mut self, idx: usize) {
        if !self.enabled {
            return;
        }
        if let Some(Some(bar)) = self.chunk_bars.get_mut(idx) {
            bar.finish_with_message("done");
        }
    }

    pub fn fail_chunk(&mut self, idx: usize, err: &str) {
        if !self.enabled {
            return;
        }
        if let Some(Some(bar)) = self.chunk_bars.get_mut(idx) {
            bar.finish_with_message(format!("failed: {err}"));
        }
    }

    pub fn finish_all(&mut self, total: u64) {
        if !self.enabled {
            return;
        }
        self.total.finish();
        if total > 0 {
            self.total.set_position(total);
            self.total.set_message("complete");
        }
    }

    pub fn abandon(&mut self) {
        if self.enabled {
            self.total.abandon();
        }
    }
}

impl Drop for ProgressUi {
    fn drop(&mut self) {
        if self.enabled {
            // Ensure terminal stays clean even on error paths.
            let _ = &self.multi;
            self.total.abandon();
        }
    }
}

/// Human-readable quick summary printed after a session finishes.
pub fn summary_line(filename: &str, bytes: u64, elapsed: Duration) -> String {
    let secs = elapsed.as_secs_f64().max(0.001);
    format!(
        "{filename}: {} in {:.1}s ({}/s)",
        human::human_bytes(bytes),
        secs,
        human::human_bytes((bytes as f64 / secs) as u64)
    )
}
