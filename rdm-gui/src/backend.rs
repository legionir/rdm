//! Bridge between the egui front-end and the `rdm` library.
//!
//! Everything the CLI can do is available here:
//!
//! | CLI                    | Backend method              |
//! |------------------------|-----------------------------|
//! | `rdm download <URL>`   | [`Backend::start_download`] |
//! | `rdm resume <ID>`      | [`Backend::resume`]         |
//! | `rdm pause <ID>`       | [`Backend::pause`]          |
//! | `rdm cancel <ID>`      | [`Backend::cancel`]         |
//! | `rdm list`             | [`Backend::list`]           |
//! | `rdm info <ID>`        | [`Backend::chunks`] / [`Backend::events`] / [`Backend::snapshot_json`] |
//! | `rdm remove <ID>`      | [`Backend::remove`]         |
//!
//! Downloads run on an embedded multi-threaded Tokio runtime, so the GUI is a
//! self-contained application: the `rdm` binary does not need to be installed.
//! Pause/cancel are honoured through the same database "control channel" the
//! CLI uses, which means a GUI can steer a download started from a terminal
//! and vice versa.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};

use rdm::downloader::engine::{Engine, EngineOptions};
use rdm::models::{ChunkRecord, DownloadRecord, DownloadState};
use rdm::storage::database::Storage;
use rdm::storage::metadata::sidecar_path;
use rdm::utils::human;

/// Everything a "start a transfer" request needs; mirrors `rdm download`.
#[derive(Debug, Clone)]
pub struct StartRequest {
    pub url: String,
    /// Output file *or* directory. Empty means "current directory".
    pub output: String,
    pub connections: u16,
    pub retries: u32,
    /// Minimum chunk size, human syntax (`1MiB`, `4MB`, …).
    pub chunk_size: String,
    /// Optional rate cap, human syntax (`5MB/s`).
    pub max_speed: String,
    /// Optional `sha256:<hex>` verification.
    pub checksum: String,
    pub user_agent: String,
    pub timeout_secs: u64,
    pub resume: bool,
    pub force: bool,
}

impl Default for StartRequest {
    fn default() -> Self {
        StartRequest {
            url: String::new(),
            output: String::new(),
            connections: 8,
            retries: 5,
            chunk_size: "1MiB".to_string(),
            max_speed: String::new(),
            checksum: String::new(),
            user_agent: String::new(),
            timeout_secs: 30,
            resume: false,
            force: false,
        }
    }
}

/// A transfer waiting for a free slot. It has no database row yet — the
/// engine creates that when the job actually starts.
#[derive(Debug, Clone)]
pub struct PendingJob {
    pub seq: u64,
    pub label: String,
    pub url: String,
    pub output: String,
    opts: EngineOptions,
    key: String,
}

/// Message pushed from a background job to the UI thread.
#[derive(Debug, Clone)]
pub enum BackendEvent {
    Info(String),
    Warn(String),
    Error(String),
}

impl BackendEvent {
    pub fn text(&self) -> &str {
        match self {
            BackendEvent::Info(s) | BackendEvent::Warn(s) | BackendEvent::Error(s) => s,
        }
    }
}

pub struct Backend {
    rt: tokio::runtime::Runtime,
    storage: Storage,
    data_dir: PathBuf,
    tx: Sender<BackendEvent>,
    rx: Receiver<BackendEvent>,
    /// Number of engines currently running inside this process.
    active: Arc<AtomicUsize>,
    /// `url\u{1}output` keys that are mid-launch, to stop double clicks.
    launching: Arc<Mutex<HashSet<String>>>,
    /// Download ids whose engine runs inside *this* process. Only these are
    /// touched on shutdown; transfers driven by a CLI in another terminal are
    /// left alone.
    owned: Arc<Mutex<HashSet<i64>>>,
    /// FIFO of transfers waiting for a free concurrency slot.
    queue: Arc<Mutex<VecDeque<PendingJob>>>,
    next_seq: AtomicU64,
}

impl Backend {
    pub fn new(data_dir: &Path) -> Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("rdm-gui")
            .build()
            .context("cannot start the async runtime")?;
        let storage = open_storage(data_dir)?;
        let (tx, rx) = channel();
        Ok(Backend {
            rt,
            storage,
            data_dir: data_dir.to_path_buf(),
            tx,
            rx,
            active: Arc::new(AtomicUsize::new(0)),
            launching: Arc::new(Mutex::new(HashSet::new())),
            owned: Arc::new(Mutex::new(HashSet::new())),
            queue: Arc::new(Mutex::new(VecDeque::new())),
            next_seq: AtomicU64::new(1),
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn db_path(&self) -> PathBuf {
        self.storage.db_path().to_path_buf()
    }

    /// Point the GUI at another `--data-dir` without restarting.
    pub fn switch_data_dir(&mut self, data_dir: &Path) -> Result<()> {
        if self.active.load(Ordering::SeqCst) > 0 {
            bail!("cannot switch data dir while downloads are running in this window");
        }
        self.storage = open_storage(data_dir)?;
        self.data_dir = data_dir.to_path_buf();
        Ok(())
    }

    pub fn active_jobs(&self) -> usize {
        self.active.load(Ordering::SeqCst)
    }

    /// Ask every transfer owned by this window to pause, then wait (bounded)
    /// for its engine to flush state. Called when the window closes so records
    /// do not stay stuck in `running` — the CLI does the same on Ctrl+C.
    ///
    /// Downloads started elsewhere (another window, a terminal) are never
    /// touched.
    pub fn shutdown(&self, grace: Duration) -> usize {
        self.clear_queue();
        let ids: Vec<i64> = {
            let guard = self.owned.lock().unwrap();
            guard.iter().copied().collect()
        };
        let mut asked = 0usize;
        for id in &ids {
            if let Ok(Some(row)) = self.storage.get_download_by_id(*id) {
                if row.state.active() {
                    let _ = self
                        .storage
                        .update_state(row.id, DownloadState::Paused, None);
                    let _ = self
                        .storage
                        .log_event(row.id, "info", "window closing — pausing");
                    asked += 1;
                }
            }
        }
        let deadline = std::time::Instant::now() + grace;
        while self.active_jobs() > 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
        // Whatever could not stop in time is marked interrupted, so the row
        // offers Resume instead of pretending to run.
        for id in &ids {
            if let Ok(Some(row)) = self.storage.get_download_by_id(*id) {
                if row.state.active() {
                    let _ = self
                        .storage
                        .update_state(row.id, DownloadState::Interrupted, None);
                    let _ = self.storage.log_event(
                        row.id,
                        "warn",
                        "window closed while running — marked interrupted",
                    );
                }
            }
        }
        asked
    }

    pub fn drain_events(&self) -> Vec<BackendEvent> {
        self.rx.try_iter().collect()
    }

    // ------------------------------------------------------------ read side

    /// `rdm list`
    pub fn list(&self) -> Result<Vec<DownloadRecord>> {
        self.storage.list_downloads()
    }

    pub fn get(&self, id: i64) -> Result<Option<DownloadRecord>> {
        self.storage.get_download_by_id(id)
    }

    /// Chunk table shown by `rdm info`.
    pub fn chunks(&self, id: i64) -> Result<Vec<ChunkRecord>> {
        self.storage.get_chunks(id)
    }

    /// `recent events` section of `rdm info`.
    pub fn events(&self, id: i64, limit: i64) -> Result<Vec<(String, String, i64)>> {
        self.storage.recent_events(id, limit)
    }

    /// `rdm info --json`
    pub fn snapshot_json(&self, id: i64) -> Result<String> {
        let meta = self
            .storage
            .snapshot(id)?
            .ok_or_else(|| anyhow!("download {id} no longer exists"))?;
        Ok(serde_json::to_string_pretty(&meta)?)
    }

    // ----------------------------------------------------------- write side

    /// `rdm download <URL> [options]` — the job is queued and started as soon
    /// as a concurrency slot is free.
    pub fn start_download(&self, req: &StartRequest, max_concurrent: usize) -> Result<u64> {
        let opts = self.engine_options(req)?;
        let label = req.url.clone();
        self.enqueue(opts, label, max_concurrent)
    }

    /// `rdm resume <ID>` — reuses the stored URL/output/connection count and
    /// lets the caller override the transfer knobs (the CLI cannot do that).
    pub fn resume(&self, id: i64, defaults: &StartRequest, max_concurrent: usize) -> Result<u64> {
        let row = self.require(id)?;
        match row.state {
            DownloadState::Completed => {
                bail!("download {} is already completed", row.public_id)
            }
            s if s.terminal() && s != DownloadState::Cancelled => {
                bail!(
                    "download {} is in state {} and cannot be resumed",
                    row.public_id,
                    s
                )
            }
            _ => {}
        }
        if row.output_path.is_empty() {
            bail!("download {} has no output path on record", row.public_id);
        }
        let req = StartRequest {
            url: row.url.clone(),
            output: row.output_path.clone(),
            connections: row.max_connections.max(1) as u16,
            retries: row.retries.max(0) as u32,
            user_agent: row
                .user_agent
                .clone()
                .unwrap_or_else(|| defaults.user_agent.clone()),
            resume: true,
            force: false,
            checksum: match (&row.checksum_algorithm, &row.checksum_expected) {
                (Some(a), Some(e)) => format!("{a}:{e}"),
                _ => String::new(),
            },
            ..defaults.clone()
        };
        let mut opts = self.engine_options(&req)?;
        // The stored output path is already a concrete file.
        opts.output = Some(PathBuf::from(&row.output_path));
        self.enqueue(opts, row.public_id.clone(), max_concurrent)
    }

    /// `rdm download --force` on an existing record: wipe chunks and refetch.
    pub fn restart(&self, id: i64, defaults: &StartRequest, max_concurrent: usize) -> Result<u64> {
        let row = self.require(id)?;
        if row.state.active() {
            bail!(
                "download {} is still running; pause or cancel it first",
                row.public_id
            );
        }
        let req = StartRequest {
            url: row.url.clone(),
            output: row.output_path.clone(),
            connections: row.max_connections.max(1) as u16,
            retries: row.retries.max(0) as u32,
            resume: false,
            force: true,
            ..defaults.clone()
        };
        let mut opts = self.engine_options(&req)?;
        opts.output = Some(PathBuf::from(&row.output_path));
        self.enqueue(opts, row.public_id.clone(), max_concurrent)
    }

    /// `rdm pause <ID>`
    pub fn pause(&self, id: i64) -> Result<String> {
        let row = self.require(id)?;
        if row.state.active() {
            self.storage
                .update_state(row.id, DownloadState::Paused, None)?;
            self.storage.log_event(row.id, "info", "pause requested")?;
            Ok(format!(
                "pause requested for {} ({})",
                row.public_id, row.filename
            ))
        } else if row.state.resumable() || row.state == DownloadState::Paused {
            Ok(format!(
                "download {} is already {} — nothing to pause",
                row.public_id, row.state
            ))
        } else {
            bail!(
                "download {} is {} and cannot be paused",
                row.public_id,
                row.state
            )
        }
    }

    /// `rdm cancel <ID>`
    pub fn cancel(&self, id: i64) -> Result<String> {
        let row = self.require(id)?;
        if row.state.active() {
            self.storage
                .update_state(row.id, DownloadState::Cancelled, None)?;
            self.storage.log_event(row.id, "warn", "cancel requested")?;
            Ok(format!(
                "cancel requested for {} ({}) — chunk data retained",
                row.public_id, row.filename
            ))
        } else if row.state.resumable() || row.state == DownloadState::Cancelled {
            self.storage
                .update_state(row.id, DownloadState::Cancelled, None)?;
            self.storage.log_event(row.id, "warn", "download cancelled")?;
            Ok(format!(
                "download {} cancelled — use Remove ▸ purge to delete its files",
                row.public_id
            ))
        } else {
            bail!(
                "download {} is {} and cannot be cancelled",
                row.public_id,
                row.state
            )
        }
    }

    /// `rdm remove <ID> [--purge]`
    pub fn remove(&self, id: i64, purge: bool) -> Result<String> {
        let row = self.require(id)?;
        if row.state.active() {
            bail!(
                "download {} is still running; cancel it first",
                row.public_id
            );
        }
        if purge {
            let _ = std::fs::remove_file(&row.output_path);
            let _ = std::fs::remove_file(sidecar_path(Path::new(&row.output_path)));
        }
        if !row.chunk_dir.is_empty() {
            let _ = std::fs::remove_dir_all(&row.chunk_dir);
        }
        self.storage.delete_download(row.id)?;
        Ok(format!(
            "removed download {} ({}){}",
            row.public_id,
            row.filename,
            if purge { " and its files" } else { "" }
        ))
    }

    /// Bulk helper used by the toolbar: pause every active transfer.
    pub fn pause_all(&self) -> Result<usize> {
        let mut n = 0;
        for row in self.list()? {
            if row.state.active() {
                self.storage
                    .update_state(row.id, DownloadState::Paused, None)?;
                self.storage.log_event(row.id, "info", "pause requested")?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// Bulk helper: resume everything that is paused/interrupted/failed.
    pub fn resume_all(&self, defaults: &StartRequest, max_concurrent: usize) -> Result<usize> {
        let mut n = 0;
        for row in self.list()? {
            let restartable = matches!(
                row.state,
                DownloadState::Paused | DownloadState::Interrupted | DownloadState::Failed
            );
            if restartable {
                self.resume(row.id, defaults, max_concurrent)?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// Bulk helper: delete every completed record (files are kept).
    pub fn remove_completed(&self, purge: bool) -> Result<usize> {
        let mut n = 0;
        for row in self.list()? {
            if row.state == DownloadState::Completed {
                self.remove(row.id, purge)?;
                n += 1;
            }
        }
        Ok(n)
    }

    // --------------------------------------------------------------- helpers

    fn require(&self, id: i64) -> Result<DownloadRecord> {
        self.storage
            .get_download_by_id(id)?
            .ok_or_else(|| anyhow!("no download with id {id}"))
    }

    /// Translate a UI request into [`EngineOptions`], applying exactly the same
    /// validation as `rdm download` (size/speed syntax, checksum shape, …).
    pub fn engine_options(&self, req: &StartRequest) -> Result<EngineOptions> {
        let url = req.url.trim();
        if url.is_empty() {
            bail!("enter a URL first");
        }
        let parsed = url::Url::parse(url).with_context(|| format!("invalid URL {url:?}"))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            bail!("only http(s) URLs are supported, got {:?}", parsed.scheme());
        }
        if !(1..=128).contains(&req.connections) {
            bail!("connections must be between 1 and 128");
        }
        let chunk_size = human::parse_bytes(chunk_spec(&req.chunk_size)).map_err(|e| anyhow!(e))?;
        if chunk_size == 0 {
            bail!("chunk size must be greater than zero");
        }
        let max_speed = match req.max_speed.trim() {
            "" => None,
            spec => Some(human::parse_speed(spec).map_err(|e| anyhow!(e))?),
        };
        let checksum = match req.checksum.trim() {
            "" => None,
            spec => Some(parse_checksum(spec)?),
        };
        let output = match req.output.trim() {
            "" => None,
            path => Some(absolutize(Path::new(path))),
        };
        let user_agent = match req.user_agent.trim() {
            "" => rdm::network::client::DEFAULT_USER_AGENT.to_string(),
            ua => ua.to_string(),
        };
        Ok(EngineOptions {
            url: url.to_string(),
            output,
            connections: req.connections as usize,
            retries: req.retries,
            chunk_size,
            max_speed,
            resume: req.resume,
            force: req.force,
            // Never draw indicatif bars from a windowed process.
            no_progress: true,
            connect_timeout: Duration::from_secs(req.timeout_secs.clamp(1, 3600)),
            user_agent,
            checksum,
            data_dir: self.data_dir.clone(),
        })
    }

    // ------------------------------------------------------------ scheduling

    /// Put a job in the queue and start it if a slot is free.
    ///
    /// `max_concurrent == 0` means "no limit".
    fn enqueue(&self, opts: EngineOptions, label: String, max_concurrent: usize) -> Result<u64> {
        let seq = self.push_job(opts, label)?;
        self.pump(max_concurrent);
        Ok(seq)
    }

    /// Register a job at the back of the queue without starting anything.
    fn push_job(&self, opts: EngineOptions, label: String) -> Result<u64> {
        let key = job_key(&opts);
        {
            let mut launching = self.launching.lock().unwrap();
            if !launching.insert(key.clone()) {
                bail!("that download is already queued or running");
            }
        }
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let job = PendingJob {
            seq,
            label,
            url: opts.url.clone(),
            output: opts
                .output
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            opts,
            key,
        };
        self.queue.lock().unwrap().push_back(job);
        Ok(seq)
    }

    /// Start queued jobs while the concurrency budget allows it. Cheap enough
    /// to call once per frame.
    pub fn pump(&self, max_concurrent: usize) -> usize {
        let mut started = 0usize;
        loop {
            if !slot_available(self.active_jobs(), max_concurrent) {
                break;
            }
            let job = match self.queue.lock().unwrap().pop_front() {
                Some(job) => job,
                None => break,
            };
            self.spawn_engine(job);
            started += 1;
        }
        started
    }

    /// Jobs still waiting for a slot, in run order.
    pub fn pending(&self) -> Vec<PendingJob> {
        self.queue.lock().unwrap().iter().cloned().collect()
    }

    pub fn pending_len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    /// Drop one waiting job (it never touched the database).
    pub fn cancel_pending(&self, seq: u64) -> Option<PendingJob> {
        let mut queue = self.queue.lock().unwrap();
        let idx = queue.iter().position(|job| job.seq == seq)?;
        let job = queue.remove(idx)?;
        self.launching.lock().unwrap().remove(&job.key);
        Some(job)
    }

    /// Drop every waiting job; running transfers are untouched.
    pub fn clear_queue(&self) -> usize {
        let drained: Vec<PendingJob> = self.queue.lock().unwrap().drain(..).collect();
        let mut launching = self.launching.lock().unwrap();
        for job in &drained {
            launching.remove(&job.key);
        }
        drained.len()
    }

    fn spawn_engine(&self, job: PendingJob) {
        let PendingJob {
            opts, label, key, ..
        } = job;
        let tx = self.tx.clone();
        let active = Arc::clone(&self.active);
        let launching = Arc::clone(&self.launching);
        let owned = Arc::clone(&self.owned);
        let storage = self.storage.clone();
        let lookup_url = opts.url.clone();
        let lookup_output = opts.output.clone();
        active.fetch_add(1, Ordering::SeqCst);
        let _ = tx.send(BackendEvent::Info(format!("started {label}")));
        self.rt.spawn(async move {
            // The engine allocates (or finds) the database row itself, so the
            // id is discovered by watching the database for a moment.
            let slot: Arc<Mutex<Option<i64>>> = Arc::new(Mutex::new(None));
            let tracker = tokio::spawn(track_download_id(
                storage,
                lookup_url,
                lookup_output,
                Arc::clone(&owned),
                Arc::clone(&slot),
            ));
            let result = match Engine::new(opts) {
                Ok(engine) => engine.run().await,
                Err(err) => Err(err),
            };
            tracker.abort();
            let tracked = *slot.lock().unwrap();
            if let Some(id) = tracked {
                owned.lock().unwrap().remove(&id);
            }
            if let Ok(outcome) = &result {
                owned.lock().unwrap().remove(&outcome.download_id);
            }
            active.fetch_sub(1, Ordering::SeqCst);
            launching.lock().unwrap().remove(&key);
            let event = match result {
                Ok(outcome) => {
                    let bytes = human::human_bytes(outcome.bytes);
                    let secs = outcome.elapsed.as_secs_f64();
                    match outcome.state {
                        DownloadState::Completed => BackendEvent::Info(format!(
                            "{} completed — {} in {:.1}s → {}",
                            outcome.public_id,
                            bytes,
                            secs,
                            outcome.output.display()
                        )),
                        DownloadState::Paused => BackendEvent::Warn(format!(
                            "{} paused at {} — press Resume to continue",
                            outcome.public_id, bytes
                        )),
                        DownloadState::Cancelled => BackendEvent::Warn(format!(
                            "{} cancelled at {}",
                            outcome.public_id, bytes
                        )),
                        DownloadState::Failed => BackendEvent::Error(format!(
                            "{} failed at {} — fix the issue and press Resume",
                            outcome.public_id, bytes
                        )),
                        other => {
                            BackendEvent::Info(format!("{} — {}", outcome.public_id, other))
                        }
                    }
                }
                Err(err) => BackendEvent::Error(format!("{label}: {err:#}")),
            };
            let _ = tx.send(event);
        });
    }
}

/// May another engine start? `max_concurrent == 0` means "no limit".
fn slot_available(active: usize, max_concurrent: usize) -> bool {
    max_concurrent == 0 || active < max_concurrent
}

/// De-duplication key for a job: the URL plus its resolved output (if known).
fn job_key(opts: &EngineOptions) -> String {
    format!(
        "{}\u{1}{}",
        opts.url,
        opts.output
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    )
}

fn open_storage(data_dir: &Path) -> Result<Storage> {
    Storage::open(&data_dir.join("metadata.db"))
        .with_context(|| format!("cannot open metadata database in {}", data_dir.display()))
}

fn chunk_spec(spec: &str) -> &str {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        "1MiB"
    } else {
        trimmed
    }
}

/// Make a user-entered path absolute so the engine's containment check
/// (which is relative to the process CWD) never rejects it.
pub fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

/// Same rules as the CLI's `--checksum sha256:<hex>`.
pub fn parse_checksum(spec: &str) -> Result<(String, String)> {
    let (algo, hex_value) = spec
        .split_once(':')
        .ok_or_else(|| anyhow!("checksum must look like `sha256:<hex>`"))?;
    let algo = algo.to_ascii_lowercase();
    if algo != "sha256" {
        bail!("unsupported checksum algorithm {algo:?} (supported: sha256)");
    }
    let hex_value = hex_value.trim();
    if hex_value.len() != 64 || !hex_value.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("sha256 digest must be 64 hex characters");
    }
    Ok((algo, hex_value.to_string()))
}

/// Watch the database until the engine's row shows up, then register it as
/// "owned by this window".
async fn track_download_id(
    storage: Storage,
    url: String,
    output: Option<PathBuf>,
    owned: Arc<Mutex<HashSet<i64>>>,
    slot: Arc<Mutex<Option<i64>>>,
) {
    for _ in 0..240 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let found = match &output {
            Some(path) => storage
                .find_download_by_url_output(&url, path)
                .ok()
                .flatten(),
            None => storage.list_downloads().ok().and_then(|rows| {
                rows.into_iter()
                    .filter(|r| r.url == url)
                    .max_by_key(|r| r.updated_at)
            }),
        };
        if let Some(row) = found {
            owned.lock().unwrap().insert(row.id);
            *slot.lock().unwrap() = Some(row.id);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_well_formed_sha256() {
        let hex = "a".repeat(64);
        let (algo, digest) = parse_checksum(&format!("SHA256:{hex}")).unwrap();
        assert_eq!(algo, "sha256");
        assert_eq!(digest, hex);
    }

    #[test]
    fn rejects_bad_checksums() {
        assert!(parse_checksum("deadbeef").is_err());
        assert!(parse_checksum("md5:abc").is_err());
        assert!(parse_checksum("sha256:xyz").is_err());
        assert!(parse_checksum(&format!("sha256:{}", "z".repeat(64))).is_err());
    }

    #[test]
    fn job_keys_separate_url_from_output() {
        let mut a = EngineOptions::default();
        a.url = "https://example.com/f.bin".to_string();
        a.output = Some(PathBuf::from("/tmp/f.bin"));
        let mut b = a.clone();
        b.output = Some(PathBuf::from("/tmp/other.bin"));
        let mut c = a.clone();
        c.output = None;
        assert_ne!(job_key(&a), job_key(&b));
        assert_ne!(job_key(&a), job_key(&c));
        assert_eq!(job_key(&a), job_key(&a.clone()));
    }

    #[test]
    fn absolutize_leaves_absolute_paths_alone() {
        let absolute = Path::new(if cfg!(windows) { "C:\\tmp\\x" } else { "/tmp/x" });
        assert_eq!(absolutize(absolute), absolute.to_path_buf());
        assert!(absolutize(Path::new("relative/x")).is_absolute());
    }

    #[test]
    fn chunk_spec_falls_back_to_one_mib() {
        assert_eq!(chunk_spec("  "), "1MiB");
        assert_eq!(chunk_spec(" 4MB "), "4MB");
    }

    fn backend() -> Backend {
        use std::sync::atomic::AtomicUsize;
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "rdm-gui-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        Backend::new(&dir).expect("backend")
    }

    #[test]
    fn engine_options_mirror_the_cli_validation() {
        let backend = backend();
        let mut request = StartRequest::default();
        request.url = "https://example.com/file.zip".to_string();
        request.chunk_size = "2MiB".to_string();
        request.max_speed = "5MB/s".to_string();
        let opts = backend.engine_options(&request).expect("valid request");
        assert_eq!(opts.chunk_size, 2 * 1024 * 1024);
        assert_eq!(opts.max_speed, Some(5_000_000));
        assert!(opts.no_progress, "a windowed app must not draw progress bars");

        request.url = "ftp://example.com/x".to_string();
        assert!(backend.engine_options(&request).is_err());

        request.url = "https://example.com/x".to_string();
        request.connections = 0;
        assert!(backend.engine_options(&request).is_err());
    }

    #[test]
    fn slot_accounting() {
        assert!(slot_available(0, 1));
        assert!(!slot_available(1, 1));
        assert!(slot_available(9, 0), "0 means unlimited");
        assert!(slot_available(2, 3));
    }

    #[test]
    fn queue_is_fifo_and_droppable() {
        let backend = backend();
        let mut request = StartRequest::default();
        request.output = std::env::temp_dir().display().to_string();

        let mut seqs = Vec::new();
        for name in ["a", "b", "c"] {
            request.url = format!("https://example.invalid/{name}.bin");
            let opts = backend.engine_options(&request).unwrap();
            seqs.push(backend.push_job(opts, name.to_string()).unwrap());
        }
        assert_eq!(backend.pending_len(), 3);
        let labels: Vec<String> = backend.pending().iter().map(|j| j.label.clone()).collect();
        assert_eq!(labels, vec!["a", "b", "c"]);

        let dropped = backend.cancel_pending(seqs[1]).expect("second job");
        assert_eq!(dropped.label, "b");
        assert!(backend.cancel_pending(seqs[1]).is_none(), "already gone");
        let labels: Vec<String> = backend.pending().iter().map(|j| j.label.clone()).collect();
        assert_eq!(labels, vec!["a", "c"]);

        assert_eq!(backend.clear_queue(), 2);
        assert_eq!(backend.pending_len(), 0);
    }

    #[test]
    fn duplicate_jobs_are_rejected() {
        let backend = backend();
        let mut request = StartRequest::default();
        request.url = "https://example.invalid/dup.bin".to_string();
        request.output = std::env::temp_dir().join("dup.bin").display().to_string();
        let first = backend.engine_options(&request).unwrap();
        let second = backend.engine_options(&request).unwrap();
        backend.push_job(first, "dup".to_string()).unwrap();
        assert!(backend.push_job(second, "dup".to_string()).is_err());
    }

    #[test]
    fn dropping_a_queued_job_frees_its_key() {
        let backend = backend();
        let mut request = StartRequest::default();
        request.url = "https://example.invalid/again.bin".to_string();
        request.output = std::env::temp_dir().join("again.bin").display().to_string();
        let opts = backend.engine_options(&request).unwrap();
        let seq = backend.push_job(opts, "again".to_string()).unwrap();
        backend.cancel_pending(seq).unwrap();
        let opts = backend.engine_options(&request).unwrap();
        assert!(
            backend.push_job(opts, "again".to_string()).is_ok(),
            "the de-duplication key must be released when a job is dropped"
        );
    }
}
