//! Per-connection download worker.
//!
//! A worker owns one HTTP connection and downloads one chunk at a time. Bytes
//! are appended durably to the chunk's `.tmp` file; progress is checkpointed
//! to SQLite periodically, and errors are retried with exponential backoff.
//! The engine can push `Adjust` (dynamic chunk split) and `Cancel` commands
//! while a transfer is in flight.

use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use reqwest::header::RANGE;
use reqwest::StatusCode;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::downloader::chunk::ChunkSpec;
use crate::models::ChunkStatus;
use crate::network::range::{classify_range_response, RangeResponse};
use crate::storage::database::Storage;
use crate::utils::rate::RateLimiter;

pub const WRITE_WINDOW: usize = 256 * 1024; // 256 KiB per body frame
const PERSIST_EVERY_BYTES: u64 = 512 * 1024;
const PROGRESS_EVERY_MS: u64 = 250;

/// Commands the engine sends to a worker.
#[derive(Debug)]
pub enum WorkerMsg {
    /// Start downloading `spec`; `offset` is durable progress (chunk-relative).
    Assign { spec: ChunkSpec, offset: u64 },
    /// Shrink the active chunk's effective end (dynamic split).
    Adjust { end: u64 },
    /// Stop as soon as possible and report a `Cancelled` event.
    Cancel,
}

/// Events a worker reports to the engine.
#[derive(Debug)]
pub enum WorkerEvent {
    Progress {
        worker: usize,
        chunk_id: i64,
        downloaded: u64,
    },
    Completed {
        worker: usize,
        chunk_id: i64,
        bytes: u64,
    },
    Failed {
        worker: usize,
        chunk_id: i64,
        error: String,
        /// True when retrying the chunk (not the whole transfer) can help.
        retryable: bool,
    },
    Cancelled {
        worker: usize,
        chunk_id: i64,
        downloaded: u64,
    },
}

/// Per-worker runtime.
pub struct Worker {
    pub index: usize,
    pub client: reqwest::Client,
    pub url: String,
    pub storage: Storage,
    pub rate: Arc<RateLimiter>,
    pub token: CancellationToken,
    pub retries: u32,
    pub backoff_base: Duration,
    /// Effective end override set by the engine when the chunk is split.
    /// Survives retries across reconnect attempts; reset on each new Assign.
    boundary_override: Option<u64>,
    rx: mpsc::UnboundedReceiver<WorkerMsg>,
    tx: mpsc::UnboundedSender<WorkerEvent>,
}

impl Worker {
    pub fn new(
        index: usize,
        client: reqwest::Client,
        url: String,
        storage: Storage,
        rate: Arc<RateLimiter>,
        token: CancellationToken,
        retries: u32,
        backoff_base: Duration,
        rx: mpsc::UnboundedReceiver<WorkerMsg>,
        tx: mpsc::UnboundedSender<WorkerEvent>,
    ) -> Self {
        Worker {
            index,
            client,
            url,
            storage,
            rate,
            token,
            retries,
            backoff_base,
            boundary_override: None,
            rx,
            tx,
        }
    }

    pub fn event_tx(&self) -> mpsc::UnboundedSender<WorkerEvent> {
        self.tx.clone()
    }

    /// Main loop.
    pub async fn run(mut self) {
        info!(worker = self.index, "worker started");
        loop {
            if self.token.is_cancelled() {
                break;
            }
            let msg = self.rx.recv().await;
            match msg {
                None => break,
                Some(WorkerMsg::Cancel) => break,
                Some(WorkerMsg::Assign { spec, offset }) => {
                    debug!(worker = self.index, chunk = spec.idx, "assigned chunk");
                    self.boundary_override = None;
                    let outcome = self.download_chunk(spec, offset).await;
                    let _ = self.tx.send(outcome);
                }
                Some(WorkerMsg::Adjust { .. }) => { /* ignored outside a transfer */ }
            }
        }
        info!(worker = self.index, "worker stopped");
    }

    async fn download_chunk(
        &mut self,
        spec: ChunkSpec,
        mut offset: u64,
    ) -> WorkerEvent {
        let mut attempt: u32 = 0;

        loop {
            if self.token.is_cancelled() {
                return WorkerEvent::Cancelled {
                    worker: self.index,
                    chunk_id: spec.id,
                    downloaded: offset,
                };
            }

            let range_start = spec.range.start + offset;
            let requested_end = self.boundary_override.unwrap_or(spec.range.end);
            if range_start > requested_end {
                // Boundary moved behind us; finish as completed.
                return WorkerEvent::Completed {
                    worker: self.index,
                    chunk_id: spec.id,
                    bytes: offset,
                };
            }

            match self
                .fetch_slice(&spec, range_start, requested_end, offset).await
            {
                Ok(bytes_written) => {
                    offset = bytes_written;
                    if offset as u64 >= requested_end - spec.range.start + 1 {
                        return WorkerEvent::Completed {
                            worker: self.index,
                            chunk_id: spec.id,
                            bytes: offset,
                        };
                    }
                    // Early EOF: the stream dropped; loop retries from here.
                    attempt += 1;
                    if attempt > self.retries {
                        return WorkerEvent::Failed {
                            worker: self.index,
                            chunk_id: spec.id,
                            error: format!(
                                "retries exhausted after premature disconnect ({offset} bytes)"
                            ),
                            retryable: true,
                        };
                    }
                    let delay = backoff(attempt, self.backoff_base);
                    warn!(
                        worker = self.index,
                        chunk = spec.idx,
                        "disconnect after {offset} bytes, retrying in {delay:?}"
                    );
                    let _ = self
                        .storage
                        .mark_chunk_retry(spec.id, "connection ended prematurely");
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {}
                        _ = self.token.cancelled() => {
                            return WorkerEvent::Cancelled {
                                worker: self.index,
                                chunk_id: spec.id,
                                downloaded: offset,
                            };
                        }
                    }
                }
                Err(err) => {
                    let (retryable, msg) = classify_error(&err);
                    attempt += 1;
                    if !retryable || attempt > self.retries {
                        return WorkerEvent::Failed {
                            worker: self.index,
                            chunk_id: spec.id,
                            error: msg,
                            retryable,
                        };
                    }
                    let delay = backoff(attempt, self.backoff_base);
                    warn!(
                        worker = self.index,
                        chunk = spec.idx,
                        attempt,
                        "chunk error, retrying in {delay:?}: {msg}"
                    );
                    let _ = self.storage.mark_chunk_retry(spec.id, &msg);
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {}
                        _ = self.token.cancelled() => {
                            return WorkerEvent::Cancelled {
                                worker: self.index,
                                chunk_id: spec.id,
                                downloaded: offset,
                            };
                        }
                    }
                }
            }
        }
    }

    /// One HTTP transaction. Writes strictly within `[range_start, requested_end]`
    /// and honors `Adjust`/`Cancel` while streaming.
    async fn fetch_slice(
        &mut self,
        spec: &ChunkSpec,
        range_start: u64,
        requested_end: u64,
        offset: u64,
    ) -> Result<u64> {
        debug!(worker = self.index, spec_id = spec.id, range_start, requested_end, offset, "fetch_slice begin");
        let mut request = self.client.get(&self.url);
        if !spec.stream {
            request = request.header(RANGE, format!("bytes={range_start}-{requested_end}"));
        }
        let resp = request.send().await.context("request failed")?;
        let status = resp.status();
        let headers = resp.headers().clone();

        match classify_range_response(status, &headers, range_start, requested_end)? {
            RangeResponse::Partial { .. } => {}
            RangeResponse::Full { .. } => {
                // Only legal for a single-stream download (whole file, no range).
                if !spec.stream && spec.range.start != 0 {
                    return Err(anyhow!(
                        "server ignored Range request (HTTP 200); this resource cannot be \
                         downloaded in segments"
                    ));
                }
            }
            RangeResponse::NotSatisfiable { claimed_size } => {
                return Err(anyhow!(
                    "server rejected range (HTTP 416); remote size: {}",
                    claimed_size
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "unknown".into())
                ));
            }
        }

        if status == StatusCode::NOT_FOUND {
            return Err(anyhow!("HTTP 404 — resource not found"));
        }
        if status == StatusCode::FORBIDDEN {
            return Err(anyhow!("HTTP 403 — access forbidden"));
        }

        let mut file = open_chunk_file(&spec.file_path, offset).await?;
        let mut stream = resp.bytes_stream();
        let mut effective_end = requested_end;
        let mut written = offset;
        let mut last_persist_bytes = offset;
        let mut last_persist = Instant::now();
        let mut last_progress = Instant::now();

        loop {
            tokio::select! {
                biased;
                _ = self.token.cancelled() => {
                    disk_checkpoint(&mut file, written, &self.storage, spec.id).await;
                    return Ok(written);
                }
                msg = self.rx.recv() => {
                    match msg {
                        Some(WorkerMsg::Adjust { end }) => {
                            // Dynamic split: stop this worker at the new boundary.
                            effective_end = effective_end.min(end);
                            self.boundary_override =
                                Some(self.boundary_override.unwrap_or(u64::MAX).min(end));
                        }
                        Some(WorkerMsg::Cancel) | None => {
                            disk_checkpoint(&mut file, written, &self.storage, spec.id).await;
                            return Ok(written);
                        }
                        Some(WorkerMsg::Assign { .. }) => { /* impossible */ }
                    }
                }
                item = stream.next() => {
                    match item {
                        None => break,
                        Some(Err(e)) => {
                            let _ = file.sync_data().await;
                            return Err(anyhow!("stream error: {e}"));
                        }
                        Some(Ok(bytes)) => {
                            if bytes.is_empty() { continue; }
                            let remaining = (effective_end - range_start + 1)
                                .saturating_sub(written - offset) as usize;
                            let take = bytes.len().min(remaining);
                            if take == 0 { break; }
                            self.rate.acquire(take).await;
                            file.write_all(&bytes[..take])
                                .await
                                .context("disk write failed (disk full or permission denied)")?;
                            written += take as u64;

                            if written - last_persist_bytes >= PERSIST_EVERY_BYTES
                                && last_persist.elapsed() > Duration::from_millis(750)
                            {
                                disk_checkpoint(&mut file, written, &self.storage, spec.id).await;
                                last_persist_bytes = written;
                                last_persist = Instant::now();
                            }
                            if last_progress.elapsed() > Duration::from_millis(PROGRESS_EVERY_MS) {
                                last_progress = Instant::now();
                                let _ = self.tx.send(WorkerEvent::Progress {
                                    worker: self.index,
                                    chunk_id: spec.id,
                                    downloaded: written,
                                });
                            }
                        }
                    }
                }
            }
        }

        let expected = effective_end - range_start + 1;
        let got = written - offset;
        // Always checkpoint: even failures leave durable progress.
        disk_checkpoint(&mut file, written, &self.storage, spec.id).await;

        if spec.stream {
            // Unknown-size stream: EOF is a normal end.
            return Ok(written);
        }
        if got < expected {
            return Err(anyhow!(
                "connection ended early ({got} of {expected} bytes)"
            ));
        }
        Ok(written)
    }
}

/// Flush + fsync + persist chunk progress to SQLite.
async fn disk_checkpoint(
    file: &mut tokio::fs::File,
    written: u64,
    storage: &Storage,
    chunk_id: i64,
) {
    let _ = file.set_len(written).await;
    let _ = file.sync_data().await;
    let _ = storage.update_chunk_progress(chunk_id, written as i64, ChunkStatus::Active);
}

async fn open_chunk_file(path: &PathBuf, offset: u64) -> Result<tokio::fs::File> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("cannot create chunk directory")?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .open(path)
        .await
        .context("cannot open chunk file")?;
    let len = file.metadata().await?.len();
    if len != offset {
        debug!("chunk file length {len} != offset {offset}; truncating file");
        file.set_len(offset).await?;
    }
    // The file cursor starts at 0: a resumed transfer must append at `offset`.
    // Without this seek the first write of a resumed chunk overwrites the
    // file from position 0 (new tail bytes land at the head, the middle keeps
    // stale data and set_len pads the end with zeros) — the assembled file
    // then has the right size but fails checksum verification.
    file.seek(SeekFrom::Start(offset))
        .await
        .context("cannot seek chunk file to resume offset")?;
    Ok(file)
}

fn backoff(attempt: u32, base: Duration) -> Duration {
    let exp = 1u64 << attempt.min(8);
    let scaled = base.saturating_mul(exp as u32);
    let jitter_ms = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
        % 250_000_000) as u64
        / 1_000_000;
    scaled.min(Duration::from_secs(60)) + Duration::from_millis(jitter_ms)
}

/// Split errors into (retryable, message).
pub fn classify_error(err: &anyhow::Error) -> (bool, String) {
    let msg = format!("{err:#}");
    let text = msg.to_lowercase();
    let retryable = [
        "connection reset",
        "connection closed",
        "timed out",
        "timeout",
        "early",
        "broken pipe",
        "unexpected eof",
        "503",
        "502",
        "500",
        "429",
        "dns",
        "connect",
        "eof",
        "reset",
        "stream error",
        "decoding response body",
        "prematurely",
    ]
    .iter()
    .any(|k| text.contains(k));
    (retryable, msg)
}
