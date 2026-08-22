//! GUI-specific view-models and actions.

use crate::models::download::{DownloadId, DownloadRecord, DownloadState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    AddDownload { url: String },
    DeleteItem { id: DownloadId },
    DeleteCompleted,
    Resume { id: DownloadId },
    Pause { id: DownloadId },
    Stop { id: DownloadId },
    ReloadSettings,
}

#[derive(Default)]
pub struct GuiState {
    pub downloads: Vec<DownloadRecord>,
    pub new_url: String,
    pub show_add: bool,
    pub selected_id: Option<DownloadId>,
    pub filter_text: String,
    pub status_message: Option<String>,
}

impl GuiState {
    pub fn new() -> Self { Self::default() }
    pub fn filtered_downloads(&self) -> Vec<&DownloadRecord> {
        let text = self.filter_text.to_lowercase();
        self.downloads.iter().filter(|r| {
            text.is_empty() || r.filename.to_lowercase().contains(&text) || r.public_id.to_lowercase().contains(&text) || r.url.to_lowercase().contains(&text)
        }).collect()
    }
    pub fn counts(&self) -> (usize, usize, usize, usize, usize) {
        let mut c = (0, 0, 0, 0, 0);
        for r in &self.downloads {
            match r.state {
                DownloadState::Completed => c.0 += 1,
                DownloadState::Queued | DownloadState::Paused => c.1 += 1,
                DownloadState::Running | DownloadState::Merging => c.2 += 1,
                DownloadState::Failed | DownloadState::Interrupted => c.3 += 1,
                _ => c.4 += 1,
            }
        }
        c
    }
}
