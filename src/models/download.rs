//! Domain types describing downloads, chunks and their lifecycle.

use std::fmt;

/// Numeric primary key of a download row.
pub type DownloadId = i64;

/// Public, human-friendly identifier shown to users (`dl-8f3c2a1b`).
pub type PublicId = String;

/// Lifecycle of a download as tracked in the metadata database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    Queued,
    Running,
    Merging,
    Paused,
    Interrupted,
    Completed,
    Cancelled,
    Failed,
}

impl DownloadState {
    pub fn as_str(&self) -> &'static str {
        match self {
            DownloadState::Queued => "queued",
            DownloadState::Running => "running",
            DownloadState::Merging => "merging",
            DownloadState::Paused => "paused",
            DownloadState::Interrupted => "interrupted",
            DownloadState::Completed => "completed",
            DownloadState::Cancelled => "cancelled",
            DownloadState::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "queued" => DownloadState::Queued,
            "running" => DownloadState::Running,
            "merging" => DownloadState::Merging,
            "paused" => DownloadState::Paused,
            "interrupted" => DownloadState::Interrupted,
            "completed" => DownloadState::Completed,
            "cancelled" => DownloadState::Cancelled,
            "failed" => DownloadState::Failed,
            _ => return None,
        })
    }

    /// States in which chunk data may still exist and be resumed.
    pub fn resumable(&self) -> bool {
        matches!(
            self,
            DownloadState::Paused
                | DownloadState::Interrupted
                | DownloadState::Failed
                | DownloadState::Running
                | DownloadState::Queued
        )
    }

    pub fn terminal(&self) -> bool {
        matches!(
            self,
            DownloadState::Completed | DownloadState::Cancelled | DownloadState::Failed
        )
    }

    /// Does this state represent a live, actively-transferring download?
    pub fn active(&self) -> bool {
        matches!(self, DownloadState::Running | DownloadState::Merging | DownloadState::Queued)
    }
}

impl fmt::Display for DownloadState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A downloaded file, including its durable metadata.
#[derive(Debug, Clone)]
pub struct DownloadRecord {
    pub id: DownloadId,
    pub public_id: PublicId,
    /// Original URL given by the user.
    pub url: String,
    /// Final URL after redirects (if any); used for range requests.
    pub effective_url: Option<String>,
    pub filename: String,
    /// Where the assembled file will be written.
    pub output_path: String,
    /// Directory holding `.tmp` chunk files.
    pub chunk_dir: String,
    pub state: DownloadState,
    pub total_size: Option<i64>,
    /// Number of bytes durably written across all chunks.
    pub downloaded_size: i64,
    pub retries: i32,
    pub max_connections: i32,
    pub user_agent: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub accept_ranges: bool,
    pub checksum_algorithm: Option<String>,
    pub checksum_expected: Option<String>,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

impl DownloadRecord {
    /// Human label used in tables and progress UI.
    pub fn label(&self) -> &str {
        &self.filename
    }
}

/// Per-chunk status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkStatus {
    Pending,
    Active,
    Completed,
    Failed,
}

impl ChunkStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChunkStatus::Pending => "pending",
            ChunkStatus::Active => "active",
            ChunkStatus::Completed => "completed",
            ChunkStatus::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "pending" => ChunkStatus::Pending,
            "active" => ChunkStatus::Active,
            "completed" => ChunkStatus::Completed,
            "failed" => ChunkStatus::Failed,
            _ => return None,
        })
    }
}

impl fmt::Display for ChunkStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Durable state of one byte range.
#[derive(Debug, Clone)]
pub struct ChunkRecord {
    pub id: i64,
    pub download_id: DownloadId,
    pub idx: i64,
    /// Inclusive start offset.
    pub start: i64,
    /// Inclusive end offset.
    pub end: i64,
    pub downloaded: i64,
    pub status: ChunkStatus,
    pub retries: i32,
    pub error: Option<String>,
    pub file_path: String,
    pub last_activity: Option<i64>,
    pub finished_at: Option<i64>,
}

impl ChunkRecord {
    /// Total byte count of this chunk (`end - start + 1`).
    pub fn len(&self) -> i64 {
        self.end - self.start + 1
    }

    /// Bytes still missing from this chunk.
    pub fn remaining(&self) -> i64 {
        (self.len() - self.downloaded).max(0)
    }

    pub fn complete(&self) -> bool {
        self.downloaded >= self.len()
    }
}

/// Progress sample used for speed statistics.
#[derive(Debug, Clone, Copy)]
pub struct ProgressSample {
    pub bytes: u64,
    pub active_connections: usize,
    pub timestamp_ms: i64,
}
