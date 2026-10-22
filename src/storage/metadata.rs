//! Filesystem layout and human-readable metadata sidecars.
//!
//! Layout:
//! ```text
//! <data-dir>/                       (default: ./.rdm)
//!   metadata.db                     (SQLite state — crash safe, WAL)
//! <output-dir>/                     (where the file lands)
//!   file.zip
//!   .rdm/<download-id>/
//!     chunk-0001.tmp ...            (temporary chunk files)
//!   metadata/
//!     file.zip.rdm.json             (human-readable metadata sidecar)
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::{ChunkRecord, DownloadRecord};

pub const METADATA_DB_NAME: &str = "metadata.db";
pub const STATE_SIDECAR_SUFFIX: &str = ".rdm.json";
pub const SIDECAR_DIR_NAME: &str = "metadata";
pub const CHUNK_DIR_PREFIX: &str = ".rdm";

/// Persistent, human-readable description of a download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadMeta {
    pub id: i64,
    pub public_id: String,
    pub url: String,
    pub effective_url: Option<String>,
    pub filename: String,
    pub output_path: String,
    pub state: String,
    pub total_size: Option<i64>,
    pub downloaded_size: i64,
    pub retries: i32,
    pub max_connections: i32,
    pub accept_ranges: bool,
    pub checksum_algorithm: Option<String>,
    pub checksum_expected: Option<String>,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub chunks: Vec<ChunkMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMeta {
    pub idx: i64,
    pub start: i64,
    pub end: i64,
    pub downloaded: i64,
    pub status: String,
    pub retries: i32,
    pub error: Option<String>,
}

impl DownloadMeta {
    pub fn from_records(dl: &DownloadRecord, chunks: &[ChunkRecord]) -> Self {
        DownloadMeta {
            id: dl.id,
            public_id: dl.public_id.clone(),
            url: dl.url.clone(),
            effective_url: dl.effective_url.clone(),
            filename: dl.filename.clone(),
            output_path: dl.output_path.clone(),
            state: dl.state.as_str().to_string(),
            total_size: dl.total_size,
            downloaded_size: dl.downloaded_size,
            retries: dl.retries,
            max_connections: dl.max_connections,
            accept_ranges: dl.accept_ranges,
            checksum_algorithm: dl.checksum_algorithm.clone(),
            checksum_expected: dl.checksum_expected.clone(),
            error: dl.error.clone(),
            created_at: dl.created_at,
            updated_at: dl.updated_at,
            started_at: dl.started_at,
            finished_at: dl.finished_at,
            chunks: chunks
                .iter()
                .map(|c| ChunkMeta {
                    idx: c.idx,
                    start: c.start,
                    end: c.end,
                    downloaded: c.downloaded,
                    status: c.status.as_str().to_string(),
                    retries: c.retries,
                    error: c.error.clone(),
                })
                .collect(),
        }
    }

    /// Write `meta` under the output directory's `metadata/` child (atomic
    /// tmp + rename).  The directory is created lazily on the first sidecar.
    pub fn write_sidecar(&self, output_path: &Path) -> std::io::Result<()> {
        let path = sidecar_path(output_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

/// `<output-dir>/metadata/<output-file-name>.rdm.json`.
pub fn sidecar_path(output: &Path) -> PathBuf {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let mut filename = output
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("download"))
        .to_os_string();
    filename.push(STATE_SIDECAR_SUFFIX);
    parent.join(SIDECAR_DIR_NAME).join(filename)
}

/// Directory that holds this download's chunk files.
pub fn chunk_dir_for(output: &Path, public_id: &str) -> PathBuf {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    parent
        .join(CHUNK_DIR_PREFIX)
        .join(public_id)
}

/// Path of chunk `idx` inside its chunk dir.
pub fn chunk_file(chunk_dir: &Path, idx: i64) -> PathBuf {
    chunk_dir.join(format!("chunk-{idx:04}.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecars_live_in_the_output_metadata_directory() {
        assert_eq!(
            sidecar_path(Path::new("downloads/file.zip")),
            PathBuf::from("downloads/metadata/file.zip.rdm.json")
        );
        assert_eq!(
            sidecar_path(Path::new("file.zip")),
            PathBuf::from("metadata/file.zip.rdm.json")
        );
    }
}
