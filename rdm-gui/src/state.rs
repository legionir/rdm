//! View-models: everything the widgets read and write, plus the action enum
//! the app loop turns into backend calls.

use std::collections::HashMap;
use std::time::Instant;

use rdm::models::{ChunkRecord, DownloadRecord, DownloadState};

use crate::backend::StartRequest;

/// One user intent produced by a view.
#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    OpenAddDialog,
    SubmitNewDownload,
    Pause(i64),
    Resume(i64),
    Cancel(i64),
    Restart(i64),
    Remove { id: i64, purge: bool },
    AskRemove(i64),
    Select(i64),
    Refresh,
    PauseAll,
    ResumeAll,
    RemoveCompleted,
    CopyToClipboard(String),
    OpenOutputFolder(i64),
    SaveSettings,
    ReloadSettings,
    ApplyDataDir,
    ClearLog,
}

/// Which detail tab is open for the selected row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    Overview,
    Chunks,
    Events,
    Json,
    Log,
}

impl DetailTab {
    pub const ALL: [DetailTab; 5] = [
        DetailTab::Overview,
        DetailTab::Chunks,
        DetailTab::Events,
        DetailTab::Json,
        DetailTab::Log,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            DetailTab::Overview => "Overview",
            DetailTab::Chunks => "Chunks",
            DetailTab::Events => "Events",
            DetailTab::Json => "JSON",
            DetailTab::Log => "App log",
        }
    }
}

/// Exponentially smoothed transfer rate derived from durable byte counters.
#[derive(Debug, Clone)]
pub struct RateTracker {
    last_bytes: i64,
    last_at: Instant,
    rate: f64,
}

impl RateTracker {
    fn new(bytes: i64) -> Self {
        RateTracker {
            last_bytes: bytes,
            last_at: Instant::now(),
            rate: 0.0,
        }
    }

    fn observe(&mut self, bytes: i64) {
        let dt = self.last_at.elapsed().as_secs_f64();
        if dt < 0.25 {
            return;
        }
        let delta = (bytes - self.last_bytes).max(0) as f64;
        let instant = delta / dt;
        self.rate = if self.rate <= 0.0 {
            instant
        } else {
            self.rate * 0.6 + instant * 0.4
        };
        self.last_bytes = bytes;
        self.last_at = Instant::now();
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub at: i64,
    pub level: &'static str,
    pub text: String,
}

/// Everything the widgets touch.
pub struct GuiState {
    pub downloads: Vec<DownloadRecord>,
    pub selected: Option<i64>,
    pub filter_text: String,
    pub state_filter: Option<DownloadState>,
    pub show_completed: bool,
    pub show_add: bool,
    pub form: StartRequest,
    pub form_error: Option<String>,
    pub detail_tab: DetailTab,
    pub chunks: Vec<ChunkRecord>,
    pub events: Vec<(String, String, i64)>,
    pub json: String,
    pub log: Vec<LogLine>,
    pub status: String,
    pub status_is_error: bool,
    pub rates: HashMap<i64, RateTracker>,
    pub pending_remove: Option<(i64, String)>,
    pub data_dir_input: String,
    pub settings_dirty: bool,
}

impl GuiState {
    pub fn new(form: StartRequest, data_dir: String) -> Self {
        GuiState {
            downloads: Vec::new(),
            selected: None,
            filter_text: String::new(),
            state_filter: None,
            show_completed: true,
            show_add: false,
            form,
            form_error: None,
            detail_tab: DetailTab::Overview,
            chunks: Vec::new(),
            events: Vec::new(),
            json: String::new(),
            log: Vec::new(),
            status: "ready".to_string(),
            status_is_error: false,
            rates: HashMap::new(),
            pending_remove: None,
            data_dir_input: data_dir,
            settings_dirty: false,
        }
    }

    pub fn push_log(&mut self, level: &'static str, text: impl Into<String>) {
        let text = text.into();
        self.status = text.clone();
        self.status_is_error = level == "error";
        self.log.push(LogLine {
            at: crate::util::now_ms(),
            level,
            text,
        });
        if self.log.len() > 500 {
            let overflow = self.log.len() - 500;
            self.log.drain(0..overflow);
        }
    }

    /// Replace the table contents and refresh the speed estimates.
    pub fn apply_rows(&mut self, rows: Vec<DownloadRecord>) {
        for row in &rows {
            let entry = self
                .rates
                .entry(row.id)
                .or_insert_with(|| RateTracker::new(row.downloaded_size));
            if row.state.active() {
                entry.observe(row.downloaded_size);
            } else {
                entry.rate = 0.0;
                entry.last_bytes = row.downloaded_size;
                entry.last_at = Instant::now();
            }
        }
        let live: Vec<i64> = rows.iter().map(|r| r.id).collect();
        self.rates.retain(|id, _| live.contains(id));
        if let Some(sel) = self.selected {
            if !live.contains(&sel) {
                self.selected = None;
            }
        }
        if self.selected.is_none() {
            self.selected = rows.first().map(|r| r.id);
        }
        self.downloads = rows;
    }

    pub fn rate_of(&self, id: i64) -> f64 {
        self.rates.get(&id).map(|r| r.rate()).unwrap_or(0.0)
    }

    pub fn selected_record(&self) -> Option<&DownloadRecord> {
        let id = self.selected?;
        self.downloads.iter().find(|r| r.id == id)
    }

    /// Rows after the search box, the state combo and the "completed" toggle.
    pub fn visible_rows(&self) -> Vec<&DownloadRecord> {
        let needle = self.filter_text.trim().to_lowercase();
        self.downloads
            .iter()
            .filter(|r| {
                if let Some(state) = self.state_filter {
                    if r.state != state {
                        return false;
                    }
                } else if !self.show_completed && r.state == DownloadState::Completed {
                    return false;
                }
                if needle.is_empty() {
                    return true;
                }
                r.filename.to_lowercase().contains(&needle)
                    || r.public_id.to_lowercase().contains(&needle)
                    || r.url.to_lowercase().contains(&needle)
                    || r.output_path.to_lowercase().contains(&needle)
            })
            .collect()
    }

    /// (completed, waiting, active, failed, other) counters for the status bar.
    pub fn counts(&self) -> (usize, usize, usize, usize, usize) {
        let mut c = (0usize, 0usize, 0usize, 0usize, 0usize);
        for r in &self.downloads {
            match r.state {
                DownloadState::Completed => c.0 += 1,
                DownloadState::Paused | DownloadState::Queued => c.1 += 1,
                DownloadState::Running | DownloadState::Merging => c.2 += 1,
                DownloadState::Failed | DownloadState::Interrupted => c.3 += 1,
                DownloadState::Cancelled => c.4 += 1,
            }
        }
        c
    }
}

/// Fraction in `0.0..=1.0` for the progress bar.
pub fn progress_of(record: &DownloadRecord) -> f32 {
    match record.total_size {
        Some(total) if total > 0 => {
            (record.downloaded_size.max(0) as f64 / total as f64).clamp(0.0, 1.0) as f32
        }
        _ => {
            if record.state == DownloadState::Completed {
                1.0
            } else {
                0.0
            }
        }
    }
}

/// Every state, in the order shown by the filter combo box.
pub const ALL_STATES: [DownloadState; 8] = [
    DownloadState::Queued,
    DownloadState::Running,
    DownloadState::Merging,
    DownloadState::Paused,
    DownloadState::Interrupted,
    DownloadState::Completed,
    DownloadState::Cancelled,
    DownloadState::Failed,
];
