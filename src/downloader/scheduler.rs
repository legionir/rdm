//! Chunk planning and resume reconstruction.

use std::path::Path;

use anyhow::{bail, Result};

use crate::downloader::chunk::{ChunkSpec, ChunkState};
use crate::models::{ChunkRecord, DownloadRecord};
use crate::network::range::{plan_chunks, ByteRange};
use crate::storage::metadata::chunk_file;

/// Clamp the per-chunk granularity so that no chunk is smaller than this.
pub const DEFAULT_MIN_CHUNK: u64 = 1 << 20; // 1 MiB

/// Build a fresh chunk plan for a download.
pub fn plan(
    download_id: i64,
    total: u64,
    streaming: bool,
    connections: usize,
    min_chunk: u64,
    chunk_dir: &Path,
) -> Vec<ChunkState> {
    if streaming {
        return vec![ChunkState::new(ChunkSpec::streaming(
            download_id,
            0,
            chunk_file(chunk_dir, 0),
        ))];
    }
    let ranges = plan_chunks(total, connections, min_chunk.max(DEFAULT_MIN_CHUNK));
    ranges
        .into_iter()
        .enumerate()
        .map(|(i, range)| {
            ChunkState::new(ChunkSpec::ranged(
                0,
                download_id,
                i as i64,
                range,
                chunk_file(chunk_dir, i as i64),
            ))
        })
        .collect()
}

/// Rebuild live chunk state from database rows (resume path).
pub fn from_records(records: &[ChunkRecord]) -> Vec<ChunkState> {
    records
        .iter()
        .map(|r| {
            let spec = ChunkSpec::ranged(
                r.id,
                r.download_id,
                r.idx,
                ByteRange {
                    start: r.start as u64,
                    end: r.end as u64,
                },
                r.file_path.clone().into(),
            );
            let mut state = ChunkState::new(spec);
            state.downloaded = r.downloaded as u64;
            state.status = r.status;
            state.retries = r.retries as u32;
            state.error = r.error.clone();
            state
        })
        .collect()
}

/// Number of bytes durable across all chunks (used for metadata).
pub fn durable_bytes(states: &[ChunkState]) -> u64 {
    states.iter().map(|c| c.downloaded).sum()
}

/// Validate that a resumed plan is sane with respect to the probed file size.
pub fn validate_plan(_records: &DownloadRecord, states: &[ChunkState], total: Option<u64>) -> Result<()> {
    if let Some(total) = total {
        let covered: u64 = states.iter().map(|c| c.end - c.spec.range.start + 1).sum();
        if covered != total {
            bail!(
                "remote file has changed (size {} vs planned {}); delete the download and restart",
                total,
                covered
            );
        }
    }
    Ok(())
}
