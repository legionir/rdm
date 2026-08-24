//! Download orchestration: probing, chunk scheduling, worker lifecycle,
//! progress reporting, pause/resume/cancel and final assembly.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::console::ProgressUi;
use crate::downloader::chunk::{ChunkSpec, ChunkTable};
use crate::downloader::scheduler::{self, DEFAULT_MIN_CHUNK};
use crate::downloader::worker::{Worker, WorkerEvent, WorkerMsg};
use crate::filesystem::merger::{compute_sha256, merge_chunks};
use crate::models::{ChunkStatus, DownloadRecord, DownloadState};
use crate::network::client::{build_client, probe, HttpOptions};
use crate::storage::database::Storage;
use crate::storage::metadata::{chunk_dir_for, chunk_file, DownloadMeta};
use crate::utils::rate::RateLimiter;

/// Configuration for one download session.
#[derive(Debug, Clone)]
pub struct EngineOptions {
    pub url: String,
    pub output: Option<PathBuf>,
    pub connections: usize,
    pub retries: u32,
    /// Minimum chunk size used by planning and dynamic splitting.
    pub chunk_size: u64,
    pub max_speed: Option<u64>,
    pub resume: bool,
    pub force: bool,
    pub no_progress: bool,
    pub connect_timeout: Duration,
    pub user_agent: String,
    /// Optional `(algorithm, expected_hex)` verification.
    pub checksum: Option<(String, String)>,
    pub data_dir: PathBuf,
}

impl Default for EngineOptions {
    fn default() -> Self {
        EngineOptions {
            url: String::new(),
            output: None,
            connections: 8,
            retries: 5,
            chunk_size: DEFAULT_MIN_CHUNK,
            max_speed: None,
            resume: false,
            force: false,
            no_progress: false,
            connect_timeout: Duration::from_secs(30),
            user_agent: crate::network::client::DEFAULT_USER_AGENT.to_string(),
            checksum: None,
            data_dir: PathBuf::from(".rdm"),
        }
    }
}

/// Result of a finished session.
#[derive(Debug)]
pub struct EngineOutcome {
    pub download_id: i64,
    pub public_id: String,
    pub state: DownloadState,
    pub bytes: u64,
    pub elapsed: Duration,
    pub output: PathBuf,
}

enum StopReason {
    Paused,
    Cancelled,
    Failed(String),
}

/// The engine. One instance per `download`/`resume` invocation.
pub struct Engine {
    opts: EngineOptions,
    storage: Storage,
    client: reqwest::Client,
}

impl Engine {
    pub fn new(opts: EngineOptions) -> Result<Self> {
        let storage = Storage::open(&opts.data_dir.join("metadata.db"))?;
        let http_opts = HttpOptions {
            connect_timeout: opts.connect_timeout,
            user_agent: opts.user_agent.clone(),
            pool_max_idle_per_host: opts.connections.max(2) + 4,
            ..Default::default()
        };
        let client = build_client(&http_opts)?;
        Ok(Engine { opts, storage, client })
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// Run the full lifecycle. Operational failures return `Err`; user-level
    /// interruptions are reported via the outcome's `state`.
    pub async fn run(self) -> Result<EngineOutcome> {
        let started = Instant::now();
        let opts = &self.opts;

        // ---- 1. validate & probe ---------------------------------------------
        let url = url::Url::parse(&opts.url)
            .with_context(|| format!("invalid URL {:?}", opts.url))?;
        if !matches!(url.scheme(), "http" | "https") {
            bail!("only http(s) URLs are supported, got {:?}", url.scheme());
        }
        if url.host_str().is_none() {
            bail!("URL has no host: {url}");
        }
        debug!(%url, "probing resource");
        let probe = probe(&self.client, url.as_str()).await?;
        let effective_url = probe.effective_url.clone();

        // ---- 2. resolve destination --------------------------------------------
        let cwd = std::env::current_dir().context("cannot determine current directory")?;
        let output = crate::utils::path::resolve_output_path(
            opts.output.as_deref(),
            &url,
            probe.disposition_filename.as_deref(),
            &cwd,
        )?;
        let output = if output.is_absolute() {
            crate::utils::path::normalize_path(&output)
        } else {
            crate::utils::path::ensure_within(&cwd, &output)?
        };
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot create output directory {}", parent.display()))?;
        }
        let filename = output
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "download".to_string());

        // ---- 3. find or create the DB row -------------------------------------
        let existing = self.storage.find_download_by_url_output(&opts.url, &output)?;
        let dl: DownloadRecord = match existing {
            Some(row) if opts.force => {
                self.storage.delete_chunks(row.id)?;
                let _ = std::fs::remove_dir_all(&row.chunk_dir);
                self.storage.update_state(row.id, DownloadState::Queued, None)?;
                self.storage.set_downloaded_bytes(row.id, 0)?;
                self.storage.log_event(row.id, "warn", "forced restart")?;
                self.storage.get_download_by_id(row.id)?.unwrap()
            }
            Some(row) if row.state == DownloadState::Completed && !opts.resume => {
                bail!(
                    "download already completed at {} (use --resume to fetch a changed file or --force to redownload)",
                    row.output_path
                );
            }
            Some(row) if !opts.resume && !opts.force => {
                bail!(
                    "an incomplete download exists (id {}, state {}); use --resume or --force",
                    row.public_id,
                    row.state
                );
            }
            Some(row) => row,
            None => {
                let row = self.storage.insert_download(
                    &opts.url,
                    &filename,
                    &output,
                    Path::new(""),
                    opts.retries as i32,
                    opts.connections as i32,
                    Some(&opts.user_agent),
                )?;
                let chunk_dir = chunk_dir_for(&output, &row.public_id);
                self.storage.set_chunk_dir(row.id, &chunk_dir)?;
                // Re-read so the in-memory record carries the canonical dir.
                self.storage
                    .get_download_by_id(row.id)?
                    .ok_or_else(|| anyhow!("download disappeared right after insert"))?
            }
        };
        let download_id = dl.id;
        let public_id = dl.public_id.clone();
        let chunk_dir = PathBuf::from(&dl.chunk_dir);
        std::fs::create_dir_all(&chunk_dir)
            .with_context(|| format!("cannot create chunk dir {}", chunk_dir.display()))?;

        // ---- 4. record server metadata ----------------------------------------
        self.storage.update_download_meta(
            download_id,
            Some(&effective_url),
            probe.size.map(|s| s as i64),
            probe.accept_ranges,
            probe.etag.as_deref(),
            probe.last_modified.as_deref(),
            opts.checksum.as_ref().map(|(a, _)| a.as_str()),
            opts.checksum.as_ref().map(|(_, e)| e.as_str()),
        )?;
        self.storage.log_event(download_id, "info", &format!("probing {url} — ok"))?;
        self.storage.update_state(download_id, DownloadState::Running, None)?;

        // ---- 5. chunk plan -----------------------------------------------------
        let dl = self
            .storage
            .get_download_by_id(download_id)?
            .ok_or_else(|| anyhow!("download disappeared during setup"))?;
        let mut table = self.setup_chunks(&dl, probe.size, &chunk_dir)?;

        if table.is_empty() {
            // Zero-byte file: nothing to download, but materialize the output.
            std::fs::write(&output, b"")
                .with_context(|| format!("cannot create {}", output.display()))?;
            self.storage.update_state(download_id, DownloadState::Completed, None)?;
            let _ = std::fs::remove_dir_all(&chunk_dir);
            let rec = self.storage.get_download_by_id(download_id)?.unwrap();
            let _ = DownloadMeta::from_records(&rec, &[]).write_sidecar(&output);
            return Ok(EngineOutcome {
                download_id,
                public_id,
                state: DownloadState::Completed,
                bytes: 0,
                elapsed: started.elapsed(),
                output,
            });
        }

        // ---- 6. UI + rate limiter ----------------------------------------------
        let mut ui = ProgressUi::new(
            &filename,
            probe.size.unwrap_or(0),
            opts.connections.max(1),
            opts.no_progress,
        );
        let rate = match opts.max_speed {
            Some(bytes) => RateLimiter::new(bytes)?.shared(),
            None => RateLimiter::unlimited().shared(),
        };

        // ---- 7. workers ---------------------------------------------------------
        let token = CancellationToken::new();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<WorkerEvent>();
        let mut worker_queues: Vec<mpsc::UnboundedSender<WorkerMsg>> = Vec::new();
        let mut handles = Vec::new();
        for i in 0..opts.connections.max(1) {
            let (msg_tx, msg_rx) = mpsc::unbounded_channel();
            worker_queues.push(msg_tx.clone());
            let worker = Worker::new(
                i,
                self.client.clone(),
                effective_url.clone(),
                self.storage.clone(),
                rate.clone(),
                token.clone(),
                opts.retries,
                Duration::from_millis(500),
                msg_rx,
                event_tx.clone(),
            );
            handles.push(tokio::spawn(worker.run()));
        }

        // ---- 8. scheduler state --------------------------------------------------
        let mut busy: Vec<Option<usize>> = vec![None; opts.connections.max(1)];
        let mut cid_to_idx: HashMap<i64, usize> = HashMap::new();
        for (i, c) in table.states().iter().enumerate() {
            if c.spec.id != 0 {
                cid_to_idx.insert(c.spec.id, i);
            }
        }
        let mut failed_attempts: HashMap<i64, u32> = HashMap::new();
        let mut fallback_done = false;
        let mut stop: Option<StopReason> = None;
        let mut ticker = tokio::time::interval(Duration::from_millis(750));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let last_control_check = Instant::now();

        self.distribute_work(
            &mut table,
            &mut busy,
            &worker_queues,
            &mut cid_to_idx,
            opts.chunk_size,
        )?;
        self.refresh_ui(&mut ui, &table, started.elapsed());

        // ---- 9. main loop ---------------------------------------------------------
        let outcome: EngineOutcome = loop {
            self.distribute_work(
                &mut table,
                &mut busy,
                &worker_queues,
                &mut cid_to_idx,
                opts.chunk_size,
            )?;

            tokio::select! {
                ev = event_rx.recv() => {
                    match ev {
                        None => {
                            // All workers went away (channels closed).
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                        Some(event) => {
                            self.handle_event(
                                event, &mut table, &mut busy, &mut cid_to_idx,
                                &mut failed_attempts, &mut fallback_done,
                                &mut stop, &download_id, &mut ui,
                            )?;
                        }
                    }
                }
                _ = ticker.tick() => {
                    let durable = table.total_downloaded();
                    let _ = self.storage.set_downloaded_bytes(download_id, durable as i64);
                    self.refresh_ui(&mut ui, &table, started.elapsed());

                    if last_control_check.elapsed() > Duration::from_millis(1000) {
                        if let Ok(Some(row)) = self.storage.get_download_by_id(download_id) {
                            match row.state {
                                DownloadState::Paused if stop.is_none() => {
                                    info!("pause requested via control channel");
                                    stop = Some(StopReason::Paused);
                                    token.cancel();
                                }
                                DownloadState::Cancelled if stop.is_none() => {
                                    info!("cancel requested via control channel");
                                    stop = Some(StopReason::Cancelled);
                                    token.cancel();
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("SIGINT — pausing (run `rdm resume {public_id}` to continue)");
                    if stop.is_none() {
                        stop = Some(StopReason::Paused);
                        token.cancel();
                    }
                }
            }

            // Success: every chunk completed.
            if table.pending_indexes().is_empty() {
                let failed = table
                    .states()
                    .iter()
                    .filter(|c| c.status == ChunkStatus::Failed)
                    .count();
                if failed > 0 && stop.is_none() {
                    let msg = table
                        .states()
                        .iter()
                        .find(|c| c.status == ChunkStatus::Failed)
                        .and_then(|c| c.error.as_deref())
                        .unwrap_or("one or more chunks failed");
                    stop = Some(StopReason::Failed(msg.to_string()));
                } else if failed == 0 {
                    break self
                        .finish(&table, &download_id, &public_id, &output, &chunk_dir, started.elapsed(), &mut ui)
                        .await?;
                }
            }

            if let Some(reason) = stop {
                let (state, err) = match reason {
                    StopReason::Paused => (DownloadState::Paused, None),
                    StopReason::Cancelled => (DownloadState::Cancelled, None),
                    StopReason::Failed(msg) => (DownloadState::Failed, Some(msg)),
                };
                self.storage.update_state(download_id, state, err.as_deref())?;
                self.storage.log_event(
                    download_id,
                    if state == DownloadState::Failed { "error" } else { "warn" },
                    match &err {
                        Some(e) => e,
                        None => match state {
                            DownloadState::Paused => "download paused — chunks preserved",
                            _ => "download cancelled",
                        },
                    },
                )?;
                ui.abandon();
                break EngineOutcome {
                    download_id,
                    public_id,
                    state,
                    bytes: table.total_downloaded(),
                    elapsed: started.elapsed(),
                    output: output.clone(),
                };
            }
        };

        // ---- 10. teardown ---------------------------------------------------------
        token.cancel();
        for q in &worker_queues {
            let _ = q.send(WorkerMsg::Cancel);
        }
        for h in handles {
            let _ = h.await;
        }
        let _ = self.write_sidecar(download_id, &output);
        Ok(outcome)
    }

    // ------------------------------------------------------------------ helpers

    fn setup_chunks(
        &self,
        dl: &DownloadRecord,
        size: Option<u64>,
        chunk_dir: &Path,
    ) -> Result<ChunkTable> {
        let has_durable_chunks = !self.storage.get_chunks(dl.id)?.is_empty();
        info!(fresh = !has_durable_chunks, state = %dl.state, "setup_chunks enter");
        if dl.state.resumable() && has_durable_chunks {
            let records = self.storage.get_chunks(dl.id)?;
            let states = scheduler::from_records(&records);
            scheduler::validate_plan(dl, &states, size)?;
            info!(count = states.len(), "resume: loaded chunk states");
            let mut table = ChunkTable::new(states);
            let fixed = table.verify_disk();
            for idx in fixed {
                self.storage
                    .log_event(dl.id, "warn", &format!("chunk {idx} had invalid data; restarting"))?;
            }
            for c in table.states() {
                let _ = self
                    .storage
                    .update_chunk_progress(c.spec.id, c.downloaded as i64, c.status);
            }
            Ok(table)
        } else {
            self.plan_fresh(dl, size, chunk_dir)
        }
    }

    fn plan_fresh(
        &self,
        dl: &DownloadRecord,
        size: Option<u64>,
        chunk_dir: &Path,
    ) -> Result<ChunkTable> {
        let (total, streaming) = match size {
            Some(0) => (0, false),
            Some(t) => (t, false),
            None => (0, true), // unknown-size single stream
        };
        if total == 0 && !streaming {
            return Ok(ChunkTable::new(Vec::new()));
        }
        // Servers that do not advertise byte ranges must not be segmented.
        let connections = if dl.accept_ranges {
            dl.max_connections.max(1) as usize
        } else {
            1
        };
        let states = scheduler::plan(
            dl.id,
            total,
            streaming,
            connections,
            self.opts.chunk_size,
            chunk_dir,
        );
        let mut table = ChunkTable::new(states);
        let snapshot: Vec<ChunkSpec> = table.states().iter().map(|c| c.spec.clone()).collect();
        for (i, spec) in snapshot.into_iter().enumerate() {
            let id = self.storage.insert_chunk(
                dl.id,
                spec.idx,
                spec.range.start as i64,
                spec.range.end as i64,
                &chunk_file(chunk_dir, spec.idx),
            )?;
            table.set_chunk_spec(i, ChunkSpec { id, ..spec });
        }
        Ok(table)
    }

    /// Rebuild the plan as a single unknown-size stream (fallback when the
    /// server rejects byte ranges).
    fn plan_fresh_unknown(&self, download_id: i64, chunk_dir: &Path) -> ChunkTable {
        let mut states = scheduler::plan(
            download_id,
            0,
            true,
            1,
            self.opts.chunk_size,
            chunk_dir,
        );
        let spec = states.drain(..).next().expect("single stream plan");
        let spec = spec.spec;
        let id = self
            .storage
            .insert_chunk(
                download_id,
                spec.idx,
                spec.range.start as i64,
                spec.range.end as i64,
                &chunk_file(chunk_dir, spec.idx),
            )
            .unwrap_or(0);
        let spec = ChunkSpec { id, ..spec };
        let mut table = ChunkTable::new(vec![crate::downloader::chunk::ChunkState::new(spec)]);
        table.verify_disk();
        table
    }

    fn distribute_work(
        &self,
        table: &mut ChunkTable,
        busy: &mut [Option<usize>],
        queues: &[mpsc::UnboundedSender<WorkerMsg>],
        cid_to_idx: &mut HashMap<i64, usize>,
        min_chunk: u64,
    ) -> Result<()> {
        let mut idle: Vec<usize> = (0..busy.len()).filter(|w| busy[*w].is_none()).collect();
        while let Some(w) = idle.pop() {
            if let Some(idx) = table.next_pending() {
                let (spec, offset) = {
                    let c = table.get(idx).ok_or_else(|| anyhow!("bad chunk index"))?;
                    (c.spec.clone(), c.downloaded)
                };
                debug!(worker = w, spec_id = spec.id, idx, "assigning chunk");
                let _ = queues[w].send(WorkerMsg::Assign { spec, offset });
                table.assign(idx, w);
                busy[w] = Some(idx);
                continue;
            }
            // Dynamic split: with an idle worker and a large active chunk,
            // hand the second half to the idle worker.
            if let Some((parent, boundary)) = table.split_candidate(min_chunk) {
                let parent_spec = table.get(parent).ok_or_else(|| anyhow!("bad parent"))?.spec.clone();
                let parent_worker = table.get(parent).and_then(|c| c.worker);
                let _child = table.split(parent, boundary).ok_or_else(|| anyhow!("split failed"))?;
                if parent_spec.id != 0 {
                    self.storage.update_chunk_end(parent_spec.id, (boundary - 1) as i64)?;
                }
                let child_idx = table.len() - 1;
                let child_spec = table.get(child_idx).ok_or_else(|| anyhow!("bad child"))?.spec.clone();
                let new_idx = table.states().iter().map(|c| c.spec.idx).max().unwrap_or(0) + 1;
                let child_id = self.storage.insert_chunk(
                    parent_spec.download_id,
                    new_idx,
                    child_spec.range.start as i64,
                    child_spec.range.end as i64,
                    &child_spec.file_path,
                )?;
                table.set_chunk_spec(child_idx, ChunkSpec { id: child_id, idx: new_idx, ..child_spec });
                cid_to_idx.insert(child_id, child_idx);
                if let Some(pw) = parent_worker {
                    if let Some(q) = queues.get(pw) {
                        let _ = q.send(WorkerMsg::Adjust { end: boundary - 1 });
                    }
                }
                if let Some(q) = queues.get(w) {
                    let spec = table.get(child_idx).ok_or_else(|| anyhow!("bad child spec"))?.spec.clone();
                    let _ = q.send(WorkerMsg::Assign { spec, offset: 0 });
                }
                table.assign(child_idx, w);
                busy[w] = Some(child_idx);
                continue;
            }
            break;
        }
        Ok(())
    }

    fn handle_event(
        &self,
        event: WorkerEvent,
        table: &mut ChunkTable,
        busy: &mut [Option<usize>],
        cid_to_idx: &mut HashMap<i64, usize>,
        failed_attempts: &mut HashMap<i64, u32>,
        fallback_done: &mut bool,
        stop: &mut Option<StopReason>,
        download_id: &i64,
        ui: &mut ProgressUi,
    ) -> Result<()> {
        match event {
            WorkerEvent::Progress { worker, chunk_id, downloaded, .. } => {
                debug!(worker, chunk_id, downloaded, "progress event");
                if let Some(&idx) = cid_to_idx.get(&chunk_id) {
                    table.progress(idx, downloaded);
                    ui.set_chunk_progress(idx, downloaded);
                }
            }
            WorkerEvent::Completed { worker, chunk_id, bytes } => {
                let Some(&idx) = cid_to_idx.get(&chunk_id) else {
                    let known: Vec<i64> = cid_to_idx.keys().copied().collect();
                    warn!(
                        worker, chunk_id, known = ?known,
                        "completion event for unknown chunk (plan replaced?) — ignored"
                    );
                    if let Some(b) = busy.get_mut(worker) {
                        *b = None;
                    }
                    return Ok(());
                };
                if table.get(idx).map(|c| c.is_stream()).unwrap_or(false) {
                    table.clamp_stream_end(idx, bytes);
                    if let Some(c) = table.get(idx) {
                        let _ = self.storage.update_chunk_end(chunk_id, c.end as i64);
                    }
                }
                table.complete(idx);
                let _ = self.storage.mark_chunk_finished(chunk_id, None);
                debug!(worker, chunk = chunk_id, bytes, "chunk completed");
                ui.finish_chunk(idx);
                if let Some(b) = busy.get_mut(worker) {
                    *b = None;
                }
            }
            WorkerEvent::Failed { worker, chunk_id, error, retryable } => {
                let attempts = failed_attempts.entry(chunk_id).or_insert(0);
                *attempts += 1;
                warn!(worker, chunk = chunk_id, "chunk failed: {error}");
                // A server that advertises ranges but answers every ranged
                // request with 200 cannot be segmented: degrade to one stream.
                if !retryable && error.contains("cannot be downloaded in segments") && !*fallback_done {
                    info!("server does not honor Range requests — switching to single stream");
                    let download_id = *download_id;
                    let chunk_dir = table.states().first().map(|c| {
                        c.spec.file_path.parent().map(|p| p.to_path_buf())
                    }).flatten();
                    if let Some(dir) = chunk_dir {
                        let _ = self.storage.delete_chunks(download_id);
                        let _ = std::fs::remove_dir_all(&dir);
                        let mut fresh = self.plan_fresh_unknown(download_id, &dir);
                        std::mem::swap(&mut fresh, table);
                        for (i, c) in table.states().iter().enumerate() {
                            if c.spec.id != 0 {
                                cid_to_idx.insert(c.spec.id, i);
                            }
                        }
                        for b in busy.iter_mut() {
                            *b = None;
                        }
                        ui.set_connections(0, 0, table.len());
                        *fallback_done = true;
                        return Ok(());
                    }
                }
                // Ignore events for chunks from an old plan (fallback replaced them).
                let Some(&idx) = cid_to_idx.get(&chunk_id) else {
                    if let Some(b) = busy.get_mut(worker) {
                        *b = None;
                    }
                    return Ok(());
                };
                let limit = self.opts.retries.max(1) + 2;
                if retryable && *attempts < limit {
                    table.release(idx);
                    let _ = self.storage.mark_chunk_retry(chunk_id, &error);
                    ui.set_chunk_message(idx, format!("retry {}/{}", *attempts, limit));
                } else {
                    table.mark_failed(idx, error.clone());
                    let _ = self.storage.mark_chunk_finished(chunk_id, Some(&error));
                    ui.fail_chunk(idx, &error);
                    if stop.is_none() {
                        *stop = Some(StopReason::Failed(error));
                        let _ = self.storage.update_state(
                            *download_id,
                            DownloadState::Failed,
                            None,
                        );
                    }
                }
                if let Some(b) = busy.get_mut(worker) {
                    *b = None;
                }
            }
            WorkerEvent::Cancelled { worker, chunk_id, downloaded } => {
                let idx = *cid_to_idx.get(&chunk_id).unwrap_or(&0);
                table.release(idx);
                let _ = self
                    .storage
                    .update_chunk_progress(chunk_id, downloaded as i64, ChunkStatus::Pending);
                if let Some(b) = busy.get_mut(worker) {
                    *b = None;
                }
            }
        }
        Ok(())
    }

    fn refresh_ui(&self, ui: &mut ProgressUi, table: &ChunkTable, elapsed: Duration) {
        let durable = table.total_downloaded();
        let remaining: u64 = table.states().iter().map(|c| c.remaining()).sum();
        ui.set_total(durable);
        ui.set_connections(table.active_count(), table.completed_count(), table.len());
        ui.set_eta(remaining, elapsed);
    }

    async fn finish(
        &self,
        table: &ChunkTable,
        download_id: &i64,
        public_id: &str,
        output: &Path,
        chunk_dir: &Path,
        elapsed: Duration,
        ui: &mut ProgressUi,
    ) -> Result<EngineOutcome> {
        self.storage.update_state(*download_id, DownloadState::Merging, None)?;
        ui.set_phase("assembling");
        info!("all chunks downloaded — assembling {}", output.display());

        // Integrity gate: chunk ranges must tile the file exactly and every
        // chunk file must match its range size before assembly.
        let mut ranges: Vec<(u64, u64, &PathBuf)> = table
            .states()
            .iter()
            .map(|c| (c.spec.range.start, c.end, &c.spec.file_path))
            .collect();
        ranges.sort_by_key(|(s, _, _)| *s);
        let mut cursor = 0u64;
        for (start, end, path) in &ranges {
            if *start != cursor {
                bail!(
                    "chunk coverage gap/overlap at offset {cursor} (chunk starts at {start});                      chunk data is inconsistent — run `rdm remove <id>` (without --purge) and restart"
                );
            }
            let on_disk = std::fs::metadata(path)
                .map(|m| m.len())
                .unwrap_or(u64::MAX);
            let expected = end - start + 1;
            if on_disk != expected {
                bail!(
                    "chunk {} size mismatch: expected {expected} bytes, found {on_disk}",
                    path.display()
                );
            }
            cursor = end + 1;
        }
        let total: u64 = ranges.iter().map(|(s, e, _)| e - s + 1).sum();
        // Merge in BYTE order: dynamic splits append children at the end of
        // the chunk table while their range sits mid-file, so table order is
        // not necessarily assembly order.
        let ordered: Vec<PathBuf> = ranges.iter().map(|(_, _, p)| (*p).clone()).collect();
        info!(chunks = ordered.len(), total, "finish: merging");

        let report = merge_chunks(ordered, output, total).await?;
        info!(size = report.bytes, "assembled file");

        if let Some((algo, expected)) = &self.opts.checksum {
            if algo.eq_ignore_ascii_case("sha256") {
                ui.set_phase("verifying sha256");
                let actual = compute_sha256(output).await?;
                if !actual.eq_ignore_ascii_case(expected) {
                    self.storage.update_state(
                        *download_id,
                        DownloadState::Failed,
                        Some("checksum mismatch"),
                    )?;
                    self.storage
                        .log_event(*download_id, "error", "checksum mismatch")?;
                    bail!("checksum mismatch: expected {expected}, got {actual}");
                }
                info!("sha256 verified: {actual}");
            }
        }

        let rm = std::fs::remove_dir_all(chunk_dir);
        info!("finish: removed chunk dir: {:?}", rm.as_ref().err().map(|e| e.to_string()));
        self.storage.update_state(*download_id, DownloadState::Completed, None)?;
        self.storage.set_downloaded_bytes(*download_id, total as i64)?;
        self.storage.log_event(*download_id, "info", "download completed")?;
        let _ = self.write_sidecar(*download_id, output);
        ui.finish_all(total);

        Ok(EngineOutcome {
            download_id: *download_id,
            public_id: public_id.to_string(),
            state: DownloadState::Completed,
            bytes: total,
            elapsed,
            output: output.to_path_buf(),
        })
    }

    fn write_sidecar(&self, id: i64, output: &Path) -> Result<()> {
        if let Some(meta) = self.storage.snapshot(id)? {
            meta.write_sidecar(output)?;
        }
        Ok(())
    }
}
