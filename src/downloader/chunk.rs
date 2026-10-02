//! Chunk domain types and the mutable scheduling table shared by worker tasks.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::models::ChunkStatus;
use crate::network::range::ByteRange;

/// Static description of one byte range to download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSpec {
    /// Database row id (0 when not yet persisted).
    pub id: i64,
    pub download_id: i64,
    pub idx: i64,
    pub range: ByteRange,
    pub file_path: PathBuf,
    /// True for unknown-size, single-stream downloads (no Range header, no resume).
    pub stream: bool,
}

impl ChunkSpec {
    pub fn ranged(id: i64, download_id: i64, idx: i64, range: ByteRange, file_path: PathBuf) -> Self {
        ChunkSpec { id, download_id, idx, range, file_path, stream: false }
    }

    pub fn streaming(download_id: i64, idx: i64, file_path: PathBuf) -> Self {
        ChunkSpec {
            id: 0,
            download_id,
            idx,
            range: ByteRange { start: 0, end: u64::MAX / 2 },
            file_path,
            stream: true,
        }
    }
}

impl ChunkSpec {
    pub fn len(&self) -> u64 {
        self.range.len()
    }
}

/// Live, scheduler-owned state of a chunk.
#[derive(Debug, Clone)]
pub struct ChunkState {
    pub spec: ChunkSpec,
    /// Bytes durably stored for this chunk (== chunk file length).
    pub downloaded: u64,
    pub status: ChunkStatus,
    /// Worker index currently assigned (if any).
    pub worker: Option<usize>,
    /// Final boundary after a dynamic split; workers never write past this.
    pub end: u64,
    pub retries: u32,
    pub error: Option<String>,
}

impl ChunkState {
    pub fn new(spec: ChunkSpec) -> Self {
        ChunkState {
            end: spec.range.end,
            spec,
            downloaded: 0,
            status: ChunkStatus::Pending,
            worker: None,
            retries: 0,
            error: None,
        }
    }

    pub fn is_stream(&self) -> bool {
        self.spec.stream
    }

    pub fn remaining(&self) -> u64 {
        self.end.saturating_sub(self.spec.range.start + self.downloaded) + 1
    }

    pub fn is_complete(&self) -> bool {
        self.status == ChunkStatus::Completed
    }
}

/// Shared scheduling table.
///
/// `std::sync::Mutex` guards quick, non-awaiting mutations; the async engine
/// holds the lock only briefly (no `.await` while locked).
pub struct ChunkTable {
    states: Vec<ChunkState>,
}

impl ChunkTable {
    pub fn new(states: Vec<ChunkState>) -> Self {
        ChunkTable { states }
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn states(&self) -> &[ChunkState] {
        &self.states
    }

    pub fn get(&self, idx: usize) -> Option<&ChunkState> {
        self.states.get(idx)
    }

    /// First chunk that is still pending (unassigned).
    pub fn next_pending(&self) -> Option<usize> {
        self.states
            .iter()
            .position(|c| c.status == ChunkStatus::Pending && c.worker.is_none())
    }

    /// Active chunks sorted by remaining bytes (largest first).
    pub fn active_sorted(&self) -> Vec<(usize, u64)> {
        let mut v: Vec<(usize, u64)> = self
            .states
            .iter()
            .enumerate()
            .filter(|(_, c)| c.status == ChunkStatus::Active)
            .map(|(i, c)| (i, c.remaining()))
            .collect();
        v.sort_by_key(|(_, remaining)| std::cmp::Reverse(*remaining));
        v
    }

    /// Candidate for a dynamic split: an active chunk with more than
    /// `4 * min_chunk` bytes remaining.
    pub fn split_candidate(&self, min_chunk: u64) -> Option<(usize, u64)> {
        let min = min_chunk.max(1);
        self.active_sorted()
            .into_iter()
            .filter(|(i, _)| !self.states[*i].is_stream())
            .find(|(_, remaining)| *remaining > 4 * min)
            .and_then(|(i, remaining)| {
                let c = &self.states[i];
                let half = (remaining / 2 / min) * min;
                if half < min {
                    return None;
                }
                let boundary = c.spec.range.start + c.downloaded + half;
                if boundary > self.states[i].end {
                    return None;
                }
                Some((i, boundary))
            })
    }

    /// Insert a new chunk split off from `parent`.
    pub fn split(&mut self, parent: usize, boundary: u64) -> Option<ChunkState> {
        let total_len = self.states.len();
        let parent_state = &mut self.states[parent];
        let old_end = parent_state.end;
        let parent_idx = parent_state.spec.idx;
        let parent_download_id = parent_state.spec.download_id;
        let parent_file = parent_state.spec.file_path.clone();
        if boundary <= parent_state.spec.range.start
            || boundary > old_end
            || old_end - boundary + 1 < 1
        {
            return None;
        }
        let mut child = ChunkState::new(ChunkSpec::ranged(
            0,
            parent_download_id,
            parent_idx + 10_000 + total_len as i64,
            ByteRange { start: boundary, end: old_end },
            parent_file.with_file_name(format!("chunk-{parent_idx:04}-{:04}.tmp", total_len + 1)),
        ));
        // Children are stateless: they start at 0 within their own range.
        child.status = ChunkStatus::Pending;
        parent_state.end = boundary - 1;
        self.states.push(child.clone());
        let new_idx = self.states.len() - 1;
        self.states[new_idx].worker = None;
        Some(child)
    }

    /// Mark a chunk assigned to worker `worker`.
    pub fn assign(&mut self, idx: usize, worker: usize) {
        if let Some(c) = self.states.get_mut(idx) {
            c.status = ChunkStatus::Active;
            c.worker = Some(worker);
        }
    }

    pub fn progress(&mut self, idx: usize, downloaded: u64) {
        if let Some(c) = self.states.get_mut(idx) {
            c.downloaded = c.downloaded.max(downloaded).min(c.end - c.spec.range.start + 1);
        }
    }

    /// Replace the spec (used to assign DB ids after insert).
    pub fn set_chunk_spec(&mut self, idx: usize, spec: ChunkSpec) {
        if let Some(c) = self.states.get_mut(idx) {
            c.spec = spec;
            c.end = c.spec.range.end;
        }
    }

    /// After a streaming chunk finishes, clamp its range to the bytes written.
    pub fn clamp_stream_end(&mut self, idx: usize, bytes: u64) {
        if let Some(c) = self.states.get_mut(idx) {
            if c.spec.stream {
                let end = c.spec.range.start + bytes.saturating_sub(1);
                c.end = end;
                c.spec.range = ByteRange { start: c.spec.range.start, end };
                c.downloaded = bytes;
            }
        }
    }


    /// Mark chunk complete after verifying its file is exactly the chunk size.
    pub fn complete(&mut self, idx: usize) {
        if let Some(c) = self.states.get_mut(idx) {
            c.status = ChunkStatus::Completed;
            c.worker = None;
        }
    }

    /// Return a chunk to the pending pool (after a failed/finished partial).
    pub fn release(&mut self, idx: usize) {
        if let Some(c) = self.states.get_mut(idx) {
            c.status = ChunkStatus::Pending;
            c.worker = None;
        }
    }

    pub fn mark_failed(&mut self, idx: usize, error: String) {
        if let Some(c) = self.states.get_mut(idx) {
            c.status = ChunkStatus::Failed;
            c.worker = None;
            c.error = Some(error);
        }
    }

    pub fn total_downloaded(&self) -> u64 {
        self.states.iter().map(|c| c.downloaded).sum()
    }

    pub fn active_count(&self) -> usize {
        self.states
            .iter()
            .filter(|c| c.status == ChunkStatus::Active)
            .count()
    }

    pub fn completed_count(&self) -> usize {
        self.states
            .iter()
            .filter(|c| c.status == ChunkStatus::Completed)
            .count()
    }

    /// Indexes of chunks that must still be downloaded.
    pub fn pending_indexes(&self) -> BTreeSet<usize> {
        self.states
            .iter()
            .enumerate()
            .filter(|(_, c)| c.status != ChunkStatus::Completed)
            .map(|(i, _)| i)
            .collect()
    }

    /// Repair pass: verify on-disk chunk size; reset mismatched chunks.
    pub fn verify_disk(&mut self) -> Vec<String> {
        let mut fixed = Vec::new();
        for c in self.states.iter_mut() {
            if c.status == ChunkStatus::Completed {
                let expected = c.end - c.spec.range.start + 1;
                match std::fs::metadata(&c.spec.file_path) {
                    Ok(md) if md.len() == expected => {}
                    _ => {
                        // Chunk was recorded complete but data is bad: restart it.
                        c.status = ChunkStatus::Pending;
                        c.downloaded = 0;
                        c.error = Some("chunk data missing/short; restarting".into());
                        fixed.push(c.spec.idx.to_string());
                    }
                }
            } else {
                // Partial: cap downloaded at actual file length.
                let actual = std::fs::metadata(&c.spec.file_path)
                    .map(|m| m.len())
                    .unwrap_or(0);
                c.downloaded = actual.min(c.end - c.spec.range.start + 1);
            }
        }
        fixed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: i64, idx: i64, start: u64, end: u64) -> ChunkSpec {
        ChunkSpec::ranged(
            id,
            1,
            idx,
            ByteRange { start, end },
            PathBuf::from(format!("c{idx}.tmp")),
        )
    }

    #[test]
    fn split_appends_child_out_of_byte_order() {
        // Three 5 MiB chunks; only the first stays active so it can be split.
        let mut table = ChunkTable::new(vec![
            ChunkState::new(spec(1, 0, 0, 5 * 1024 * 1024 - 1)),
            ChunkState::new(spec(2, 1, 5 * 1024 * 1024, 10 * 1024 * 1024 - 1)),
            ChunkState::new(spec(3, 2, 10 * 1024 * 1024, 15 * 1024 * 1024 - 1)),
        ]);
        table.assign(0, 0);
        table.assign(1, 1);
        table.assign(2, 2);
        table.complete(1);
        table.complete(2);

        let (parent, boundary) = table
            .split_candidate(1024 * 1024)
            .expect("5 MiB remainder must be splittable");
        assert_eq!(parent, 0);
        table.split(parent, boundary).unwrap();
        assert_eq!(table.len(), 4, "child is appended to the table");

        let table_order: Vec<u64> = table.states().iter().map(|c| c.spec.range.start).collect();
        let mut byte_order = table_order.clone();
        byte_order.sort_unstable();
        assert_ne!(
            table_order, byte_order,
            "assembly must sort by range, not table position"
        );
        assert_eq!(table_order.last().copied(), Some(boundary));
        assert_eq!(byte_order[1], boundary);
    }
}
