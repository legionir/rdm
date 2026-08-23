//! `rdm` command surface.
//!
//! ```text
//! rdm download <URL> [options]
//! rdm pause <ID> | resume <ID> | cancel <ID>
//! rdm list | info <ID> | remove <ID>
//! ```

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::downloader::engine::{Engine, EngineOptions, EngineOutcome};
use crate::models::{DownloadRecord, DownloadState};
use crate::storage::database::Storage;
use crate::utils::human;

#[derive(Parser, Debug)]
#[command(
    name = "rdm",
    version,
    about = "Rust Download Manager — multi-connection, resumable CLI downloader",
    long_about = "rdm downloads files over HTTP/HTTPS using concurrent segmented requests, \
                  persists state in SQLite, and resumes interrupted transfers exactly \
                  where they stopped."
)]
pub struct Opts {
    /// Directory holding the global metadata database (`metadata.db`).
    #[arg(long, global = true, default_value = ".rdm", value_name = "DIR")]
    pub data_dir: PathBuf,

    /// Increase log verbosity (`-v` info, `-vv` debug, `-vvv` trace).
    #[arg(short = 'v', long = "verbose", global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start a new download (or resume one with --resume).
    Download(DownloadArgs),
    /// Pause a download (works while the engine runs in another terminal).
    Pause(IdArgs),
    /// Resume a paused/interrupted download.
    Resume(IdArgs),
    /// Cancel a download; chunk data is kept unless removed.
    Cancel(IdArgs),
    /// List tracked downloads.
    List(ListArgs),
    /// Show detailed information about one download.
    Info(InfoArgs),
    /// Remove a download record (and optionally its files).
    Remove(RemoveArgs),
}

#[derive(Args, Debug)]
pub struct DownloadArgs {
    /// HTTP(S) URL of the file to download.
    #[arg(value_name = "URL")]
    pub url: String,

    /// Output file or directory (defaults to current directory).
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Number of parallel connections (1..=128).
    #[arg(short, long, default_value_t = 8, value_parser = clap::value_parser!(u16).range(1..=128), value_name = "N")]
    pub connections: u16,

    /// Resume an existing incomplete download for the same URL+output.
    #[arg(short, long)]
    pub resume: bool,

    /// Restart from scratch even if a download record exists.
    #[arg(short, long)]
    pub force: bool,

    /// Retries per chunk after a failure (0 disables).
    #[arg(long, default_value_t = 5, value_name = "N")]
    pub retry: u32,

    /// Give up after N seconds without progress.
    #[arg(long, default_value_t = 60, value_name = "SECS")]
    pub timeout: u64,

    /// Cap the total transfer rate (e.g. `5MB/s`, `2GiB/s`).
    #[arg(long, value_name = "SPEED")]
    pub max_speed: Option<String>,

    /// Minimum chunk size for segmented planning (e.g. `1MiB`, `4MB`).
    #[arg(long, default_value = "1MiB", value_name = "SIZE")]
    pub chunk_size: String,

    /// Verify after assembly: `sha256:<hex>`.
    #[arg(long, value_name = "ALGO:HEX")]
    pub checksum: Option<String>,

    /// Custom User-Agent header.
    #[arg(long, value_name = "AGENT")]
    pub user_agent: Option<String>,

    /// Disable the interactive progress bars.
    #[arg(long)]
    pub no_progress: bool,
}

#[derive(Args, Debug)]
pub struct IdArgs {
    /// Numeric id or public id (e.g. `dl-1a2b3c4d`).
    pub id: String,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Only show downloads in this state.
    #[arg(long, value_name = "STATE")]
    pub state: Option<String>,
    /// Show finished downloads too (default shows all).
    #[arg(long)]
    pub all: bool,
}

#[derive(Args, Debug)]
pub struct InfoArgs {
    pub id: String,
    /// Emit a machine-readable JSON snapshot.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RemoveArgs {
    pub id: String,
    /// Also delete the assembled output file and its sidecar.
    #[arg(long)]
    pub purge: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ListStateFilter {
    Running,
    Paused,
    Interrupted,
    Completed,
    Failed,
    Cancelled,
    Queued,
}

pub async fn run(opts: Opts) -> Result<u8> {
    init_logging(opts.verbose);
    match opts.command {
        Command::Download(args) => run_download(&opts.data_dir, args).await,
        Command::Pause(args) => run_pause(&opts.data_dir, &args.id).await,
        Command::Resume(args) => run_resume(&opts.data_dir, &args.id).await,
        Command::Cancel(args) => run_cancel(&opts.data_dir, &args.id).await,
        Command::List(args) => run_list(&opts.data_dir, args).await,
        Command::Info(args) => run_info(&opts.data_dir, args).await,
        Command::Remove(args) => run_remove(&opts.data_dir, args).await,
    }
}

fn init_logging(verbose: u8) {
    let default = match verbose {
        0 => "info",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| default.into());
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .try_init();
}

fn open_storage(data_dir: &PathBuf) -> Result<Storage> {
    Storage::open(&data_dir.join("metadata.db"))
        .with_context(|| format!("cannot open metadata database in {}", data_dir.display()))
}

fn lookup<'a>(storage: &'a Storage, id: &str) -> Result<DownloadRecord> {
    if let Ok(num) = id.parse::<i64>() {
        if let Some(row) = storage.get_download_by_id(num)? {
            return Ok(row);
        }
        bail!("no download with id {id}");
    }
    if let Some(row) = storage.get_download_by_public_id(id)? {
        return Ok(row);
    }
    bail!("no download with id {id}")
}

async fn run_download(data_dir: &PathBuf, args: DownloadArgs) -> Result<u8> {
    let max_speed = args
        .max_speed
        .as_deref()
        .map(human::parse_speed)
        .transpose()
        .map_err(|e| anyhow!(e))?;
    let chunk_size = human::parse_bytes(&args.chunk_size).map_err(|e| anyhow!(e))?;
    let checksum = args
        .checksum
        .as_deref()
        .map(parse_checksum)
        .transpose()?;

    let opts = EngineOptions {
        url: args.url,
        output: args.output,
        connections: args.connections as usize,
        retries: args.retry,
        chunk_size,
        max_speed,
        resume: args.resume,
        force: args.force,
        no_progress: args.no_progress,
        connect_timeout: Duration::from_secs(30),
        user_agent: args.user_agent.unwrap_or_else(|| crate::network::client::DEFAULT_USER_AGENT.to_string()),
        checksum,
        data_dir: data_dir.clone(),
    };
    let engine = Engine::new(opts)?;
    let outcome = engine.run().await?;
    report(outcome)
}

async fn run_resume(data_dir: &PathBuf, id: &str) -> Result<u8> {
    let storage = open_storage(data_dir)?;
    let row = lookup(&storage, id)?;
    match row.state {
        DownloadState::Completed => bail!("download {} is already completed", row.public_id),
        s if s.terminal() && s != DownloadState::Cancelled => {
            bail!("download {} is in state {s} and cannot be resumed", row.public_id)
        }
        _ => {}
    }
    if row.output_path.is_empty() {
        bail!("download record has no output path");
    }
    let max_speed = None; // resume keeps the stored engine defaults; no CLI override here
    let opts = EngineOptions {
        url: row.url.clone(),
        output: Some(PathBuf::from(&row.output_path)),
        connections: row.max_connections.max(1) as usize,
        retries: row.retries.max(0) as u32,
        chunk_size: 1 << 20,
        max_speed,
        resume: true,
        force: false,
        no_progress: false,
        connect_timeout: Duration::from_secs(30),
        user_agent: row.user_agent.clone().unwrap_or_else(|| crate::network::client::DEFAULT_USER_AGENT.to_string()),
        checksum: row
            .checksum_algorithm
            .clone()
            .zip(row.checksum_expected.clone()),
        data_dir: data_dir.clone(),
    };
    let engine = Engine::new(opts)?;
    let outcome = engine.run().await?;
    report(outcome)
}

async fn run_pause(data_dir: &PathBuf, id: &str) -> Result<u8> {
    let storage = open_storage(data_dir)?;
    let row = lookup(&storage, id)?;
    if row.state.active() {
        storage.update_state(row.id, DownloadState::Paused, None)?;
        storage.log_event(row.id, "info", "pause requested")?;
        println!("pause requested for {} ({})", row.public_id, row.filename);
    } else if row.state.resumable() || row.state == DownloadState::Paused {
        println!("download {} is already {} — nothing to pause", row.public_id, row.state);
    } else {
        bail!(
            "download {} is {} and cannot be paused",
            row.public_id,
            row.state
        );
    }
    Ok(0)
}

async fn run_cancel(data_dir: &PathBuf, id: &str) -> Result<u8> {
    let storage = open_storage(data_dir)?;
    let row = lookup(&storage, id)?;
    if row.state.active() {
        storage.update_state(row.id, DownloadState::Cancelled, None)?;
        storage.log_event(row.id, "warn", "cancel requested")?;
        println!("cancel requested for {} ({}) — chunk data retained", row.public_id, row.filename);
    } else if row.state.resumable() || row.state == DownloadState::Cancelled {
        storage.update_state(row.id, DownloadState::Cancelled, None)?;
        storage.log_event(row.id, "warn", "download cancelled")?;
        println!(
            "download {} cancelled — run `rdm remove {} --purge` to delete files",
            row.public_id, row.public_id
        );
    } else {
        bail!("download {} is {} and cannot be cancelled", row.public_id, row.state);
    }
    Ok(0)
}

async fn run_list(data_dir: &PathBuf, args: ListArgs) -> Result<u8> {
    let storage = open_storage(data_dir)?;
    let rows = storage.list_downloads()?;
    let filter = args
        .state
        .as_deref()
        .and_then(DownloadState::from_str)
        .map(|s| s.as_str().to_string());
    println!(
        "{:<16} {:<12} {:<8} {:>12} {:>12}  {}",
        "ID", "STATE", "CONNS", "SIZE", "DONE", "FILE"
    );
    let mut shown = 0usize;
    for row in rows {
        if let Some(f) = &filter {
            if row.state.as_str() != f {
                continue;
            }
        }
        if !args.all && row.state == DownloadState::Completed {
            continue;
        }
        let size = row
            .total_size
            .map(|s| human::human_bytes(s as u64))
            .unwrap_or_else(|| "?".into());
        println!(
            "{:<16} {:<12} {:<8} {:>12} {:>12}  {}",
            row.public_id,
            row.state,
            row.max_connections,
            size,
            human::human_bytes(row.downloaded_size.max(0) as u64),
            row.filename
        );
        shown += 1;
    }
    if shown == 0 {
        println!("(no downloads match)");
    }
    Ok(0)
}

async fn run_info(data_dir: &PathBuf, args: InfoArgs) -> Result<u8> {
    let storage = open_storage(data_dir)?;
    let row = lookup(&storage, &args.id)?;
    if args.json {
        let meta = storage
            .snapshot(row.id)?
            .ok_or_else(|| anyhow!("download vanished"))?;
        println!("{}", serde_json::to_string_pretty(&meta)?);
        return Ok(0);
    }
    println!("id:            {}", row.public_id);
    println!("state:         {}", row.state);
    println!("url:           {}", row.url);
    if let Some(eff) = &row.effective_url {
        if eff != &row.url {
            println!("effective url: {eff}");
        }
    }
    println!("file:          {}", row.filename);
    println!("output:        {}", row.output_path);
    println!("chunks dir:    {}", row.chunk_dir);
    println!(
        "size:          {}",
        row.total_size.map(|s| human::human_bytes(s as u64)).unwrap_or_else(|| "unknown".into())
    );
    println!(
        "downloaded:    {}",
        human::human_bytes(row.downloaded_size.max(0) as u64)
    );
    println!("connections:   {}", row.max_connections);
    println!("accept ranges: {}", row.accept_ranges);
    if let Some(a) = &row.checksum_algorithm {
        if let Some(e) = &row.checksum_expected {
            println!("checksum:      {a}:{e}");
        }
    }
    if let Some(e) = &row.error {
        println!("last error:    {e}");
    }
    println!("created:       {}", row.created_at);
    if let Some(t) = row.started_at {
        println!("started:       {t}");
    }
    if let Some(t) = row.finished_at {
        println!("finished:      {t}");
    }
    let chunks = storage.get_chunks(row.id)?;
    println!();
    println!("{:<6} {:>14} {:>14} {:>12} {:>9}  {}", "CHUNK", "START", "END", "DONE", "STATUS", "ERR");
    for c in &chunks {
        println!(
            "{:<6} {:>14} {:>14} {:>12} {:>9}  {}",
            c.idx,
            c.start,
            c.end,
            human::human_bytes(c.downloaded.max(0) as u64),
            c.status,
            c.error.as_deref().unwrap_or("")
        );
    }
    let events = storage.recent_events(row.id, 10)?;
    if !events.is_empty() {
        println!();
        println!("recent events:");
        for (level, msg, ts) in events {
            println!("  {ts} [{level}] {msg}");
        }
    }
    Ok(0)
}

async fn run_remove(data_dir: &PathBuf, args: RemoveArgs) -> Result<u8> {
    let storage = open_storage(data_dir)?;
    let row = lookup(&storage, &args.id)?;
    if row.state.active() {
        bail!("download {} is still running; cancel it first", row.public_id);
    }
    if args.purge {
        let _ = std::fs::remove_file(&row.output_path);
        let _ = std::fs::remove_file(crate::storage::metadata::sidecar_path(
            std::path::Path::new(&row.output_path),
        ));
        println!("removed {}", row.output_path);
    }
    let _ = std::fs::remove_dir_all(&row.chunk_dir);
    storage.delete_download(row.id)?;
    println!(
        "removed download {} ({}){}",
        row.public_id,
        row.filename,
        if args.purge { " and its files" } else { "" }
    );
    Ok(0)
}

fn parse_checksum(spec: &str) -> Result<(String, String)> {
    let (algo, hex_value) = spec
        .split_once(':')
        .ok_or_else(|| anyhow!("checksum must look like `sha256:<hex>`"))?;
    let algo = algo.to_ascii_lowercase();
    if !matches!(algo.as_str(), "sha256") {
        bail!("unsupported checksum algorithm {algo:?} (supported: sha256)");
    }
    let hex_value = hex_value.trim();
    if hex_value.len() != 64 || !hex_value.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("sha256 digest must be 64 hex characters");
    }
    Ok((algo, hex_value.to_string()))
}

fn report(outcome: EngineOutcome) -> Result<u8> {
    use crate::console::summary_line;
    let file = outcome
        .output
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "?".into());
    match outcome.state {
        DownloadState::Completed => {
            println!(
                "{} — {} ({} in {:.1}s, {})",
                outcome.public_id,
                summary_line(&file, outcome.bytes, outcome.elapsed),
                human::human_bytes(outcome.bytes),
                outcome.elapsed.as_secs_f64(),
                outcome.output.display()
            );
            Ok(0)
        }
        DownloadState::Paused => {
            println!(
                "{} — paused at {}; run `rdm resume {}` to continue",
                outcome.public_id,
                human::human_bytes(outcome.bytes),
                outcome.public_id
            );
            Ok(0)
        }
        DownloadState::Cancelled => {
            println!("{} — cancelled at {}", outcome.public_id, human::human_bytes(outcome.bytes));
            Ok(0)
        }
        DownloadState::Failed => {
            println!(
                "{} — failed at {}; run `rdm resume {}` after fixing the issue",
                outcome.public_id,
                human::human_bytes(outcome.bytes),
                outcome.public_id
            );
            Ok(1)
        }
        other => {
            println!("{} — {other}", outcome.public_id);
            Ok(0)
        }
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn parse_checksum_valid_sha256() {
        let spec = "sha256:aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899";
        let (algo, hex) = parse_checksum(spec).unwrap();
        assert_eq!(algo, "sha256");
        assert_eq!(hex.len(), 64);
    }

    #[test]
    fn parse_checksum_invalid_no_colon() {
        assert!(parse_checksum("sha256bad").is_err());
    }

    #[test]
    fn parse_checksum_invalid_short_hex() {
        assert!(parse_checksum("sha256:dead").is_err());
    }

    #[test]
    fn opts_parse_download_defaults() {
        let opts = Opts::try_parse_from(["rdm", "download", "http://example.com/file.bin"]).unwrap();
        assert!(matches!(opts.command, Command::Download(_)));
        assert_eq!(opts.data_dir, PathBuf::from(".rdm"));
    }

    #[test]
    fn opts_parse_list() {
        let opts = Opts::try_parse_from(["rdm", "list"]).unwrap();
        assert!(matches!(opts.command, Command::List(_)));
    }

    #[test]
    fn opts_parse_resume_with_resume_flag() {
        let opts = Opts::try_parse_from(["rdm", "download", "--resume", "http://example.com/f.bin"]).unwrap();
        if let Command::Download(args) = opts.command {
            assert!(args.resume);
        } else {
            panic!("expected Download");
        }
    }

    #[test]
    fn download_args_connections_range() {
        use clap::Parser;
        let args = DownloadArgs::parse_from(["http://x", "--connections", "16"]);
        assert_eq!(args.connections, 16);
    }
}
