//! End-to-end tests: a local HTTP server with Range support exercises the
//! engine (segmented download, resume after premature disconnect, single
//! stream fallback, checksum verification).

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Deterministic payload with a recognizable per-offset byte pattern.
fn payload(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 251) as u8).collect()
}

#[derive(Clone)]
struct ServerOptions {
    /// For the first `truncate_first` requests, close after this many bytes.
    truncate_at: Option<usize>,
    /// Number of requests to apply `truncate_at` to.
    truncate_first: usize,
    /// If true, the server does not advertise/support ranges (always 200).
    no_range: bool,
    /// Sleep this long between 64 KiB body writes (throttles transfers so
    /// tests can pause them mid-stream).
    pace: Option<Duration>,
    /// `(range_start, timeout)`: hold headers for a non-probe request whose
    /// range starts at `range_start` until two other ranged bodies finish
    /// (or `timeout` elapses). The stalled chunk stays at 0 bytes so an idle
    /// worker can split it — a fixed sleep is not enough on loaded Windows CI.
    stall: Option<(u64, Duration)>,
    /// Number of requests handled so far.
    requests: Arc<Mutex<usize>>,
    /// Finished `206` responses (used by `stall`).
    ranged_done: Arc<Mutex<usize>>,
}

fn server_opts() -> ServerOptions {
    ServerOptions {
        truncate_at: None,
        truncate_first: 0,
        no_range: false,
        pace: None,
        stall: None,
        requests: Arc::new(Mutex::new(0)),
        ranged_done: Arc::new(Mutex::new(0)),
    }
}

struct TestServer {
    addr: SocketAddr,
    handle: tokio::task::JoinHandle<()>,
    requests: Arc<Mutex<usize>>,
}

impl TestServer {
    async fn start(data: Arc<Vec<u8>>, opts: ServerOptions) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = opts.requests.clone();
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else { break };
                let data = data.clone();
                let opts = opts.clone();
                tokio::spawn(async move {
                    if let Err(e) = serve(&mut sock, &data, &opts).await {
                        let _ = e;
                    }
                });
            }
        });
        TestServer { addr, handle, requests }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    fn request_count(&self) -> usize {
        *self.requests.lock().unwrap()
    }
}

async fn serve(sock: &mut TcpStream, data: &[u8], opts: &ServerOptions) -> std::io::Result<()> {
    // Read headers (until CRLFCRLF).
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    loop {
        let n = sock.read(&mut tmp).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines = text.lines();
    let request_line = lines.next().unwrap_or("").to_string();
    let method = request_line.split_whitespace().next().unwrap_or("").to_string();
    let mut range: Option<(u64, u64)> = None;
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("range:") {
            let v = v.trim().trim_start_matches("bytes=");
            let (a, b) = v.split_once('-').unwrap_or(("0", "0"));
            let start: u64 = a.parse().unwrap_or(0);
            let end: u64 = if b.is_empty() {
                data.len() as u64 - 1
            } else {
                b.parse().unwrap_or(data.len() as u64 - 1)
            };
            range = Some((start, end));
        }
    }
    *opts.requests.lock().unwrap() += 1;
    // Hold a large first-chunk request until sibling ranges have finished so
    // the engine sees idle workers + an untouched active chunk. Skip 1-byte
    // probes (`bytes=0-0`) or the workers never start (deadlock).
    if let Some((stall_start, timeout)) = opts.stall {
        if let Some((s, e)) = range {
            if s == stall_start && e > s {
                let deadline = tokio::time::Instant::now() + timeout;
                loop {
                    if *opts.ranged_done.lock().unwrap() >= 2 {
                        break;
                    }
                    if tokio::time::Instant::now() >= deadline {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                // Let the engine process sibling Completions and split while
                // this request is still unanswered (downloaded stays 0).
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
    }

    let (status, start, end) = if data.is_empty() {
        // Empty resource: HEAD/GET without range -> 200 (len 0); ranged -> 416.
        match range {
            Some(_) => {
                let body = "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */0\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                sock.write_all(body.as_bytes()).await?;
                return Ok(());
            }
            None => ("200 OK", 0, 0),
        }
    } else {
        match (range, opts.no_range) {
            (Some((s, _e)), false) if s >= data.len() as u64 => {
            let body = format!(
                "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                data.len()
            );
            sock.write_all(body.as_bytes()).await?;
            return Ok(());
        }
            (Some((s, e)), false) => {
                let e = e.min(data.len() as u64 - 1);
                ("206 Partial Content", s, e)
            }
            _ => ("200 OK", 0, data.len() as u64 - 1),
        }
    };

    let chunk_len = if status.starts_with("206") {
        (end - start + 1) as usize
    } else {
        data.len()
    };
    if data.is_empty() {
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n"
        );
        sock.write_all(head.as_bytes()).await?;
        return Ok(());
    }
    let accept_ranges = if opts.no_range { "" } else { "Accept-Ranges: bytes\r\n" };
    let resp_head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {chunk_len}\r\n{accept_ranges}Content-Range: bytes {start}-{end}/{}\r\nContent-Disposition: attachment; filename=\"server-test.bin\"\r\nConnection: close\r\n\r\n",
        if status.starts_with("206") { data.len() } else { data.len() }
    );
    sock.write_all(resp_head.as_bytes()).await?;
    if method.eq_ignore_ascii_case("HEAD") {
        return Ok(());
    }

    let slice = if status.starts_with("206") {
        &data[start as usize..=end as usize]
    } else {
        data
    };
    if let Some(max) = opts.truncate_at {
        let count = *opts.requests.lock().unwrap();
        if count <= opts.truncate_first {
            let n = max.min(slice.len());
            sock.write_all(&slice[..n]).await?;
            // Close abruptly WITHOUT Content-Length satisfaction.
            mark_ranged_done(opts, status);
            return Ok(());
        }
    }
    let mut written = 0usize;
    while written < slice.len() {
        let n = (slice.len() - written).min(64 * 1024);
        sock.write_all(&slice[written..written + n]).await?;
        written += n;
        if let Some(pace) = opts.pace {
            tokio::time::sleep(pace).await;
        }
    }
    mark_ranged_done(opts, status);
    Ok(())
}

fn mark_ranged_done(opts: &ServerOptions, status: &str) {
    if status.starts_with("206") {
        *opts.ranged_done.lock().unwrap() += 1;
    }
}

/// Unique temp directory for one test.
fn test_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rdm-it-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn run_engine(
    url: String,
    output: PathBuf,
    data_dir: PathBuf,
    tweak: impl FnOnce(&mut rdm::downloader::engine::EngineOptions),
) -> anyhow::Result<rdm::downloader::engine::EngineOutcome> {
    let mut opts = rdm::downloader::engine::EngineOptions {
        url,
        output: Some(output),
        connections: 4,
        retries: 3,
        chunk_size: 1024 * 1024,
        max_speed: None,
        resume: false,
        force: false,
        no_progress: true,
        connect_timeout: Duration::from_secs(5),
        user_agent: "rdm-it".into(),
        checksum: None,
        data_dir,
    };
    tweak(&mut opts);
    let engine = rdm::downloader::engine::Engine::new(opts).unwrap();
    engine.run().await
}

fn sha256_of(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

#[tokio::test]
async fn segmented_download_matches_payload() {
    let _ = tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).try_init();
    let data = Arc::new(payload(8 * 1024 * 1024 + 12345)); // not a multiple of chunks
    let server = TestServer::start(
        data.clone(),
        server_opts(),
    )
    .await;
    let dir = test_dir("seg");
    let out = dir.join("file.bin");
    let data_dir = dir.join("data");
    let outcome = run_engine(server.url("/file.bin"), out.clone(), data_dir.clone(), |_| {}).await.unwrap();
    assert_eq!(outcome.state, rdm::models::DownloadState::Completed);
    assert_eq!(outcome.bytes, data.len() as u64);
    let got = std::fs::read(&out).unwrap();
    assert_eq!(got, *data);
    // Resumable multi-request server: >1 request proves segmentation.
    assert!(server.request_count() > 1);

    // Sidecar metadata must exist and be parseable.
    let sidecar = std::fs::read_to_string(
        Path::new(&out).with_extension("bin.rdm.json"),
    )
    .unwrap_or_default();
    assert!(sidecar.contains("\"state\": \"completed\""));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn resume_after_premature_disconnect() {
    let data = Arc::new(payload(2 * 1024 * 1024));
    // First attempt: truncate every transfer at 64KiB twice, then succeed.
    let server = TestServer::start(
        data.clone(),
        ServerOptions {
            truncate_at: Some(64 * 1024),
            truncate_first: 2,
            ..server_opts()
        },
    )
    .await;
    let dir = test_dir("resume");
    let out = dir.join("file.bin");
    let data_dir = dir.join("data");

    let outcome = run_engine(server.url("/file.bin"), out.clone(), data_dir.clone(), |_| {}).await.unwrap();
    assert_eq!(outcome.state, rdm::models::DownloadState::Completed);
    let got = std::fs::read(&out).unwrap();
    if got != *data {
        let n = got.len().min(data.len());
        let first = (0..n).find(|&i| got[i] != data[i]);
        eprintln!("MISMATCH len got={} want={} first_diff={:?}", got.len(), data.len(), first);
        let _ = std::fs::write(format!("{}-got.bin", out.display()), &got);
        let _ = std::fs::write(format!("{}-want.bin", out.display()), &*data);
    }
    assert_eq!(got, *data);

    let storage = rdm::storage::database::Storage::open(&data_dir.join("metadata.db")).unwrap();
    let dl = storage
        .find_download_by_url_output(
            &server.url("/file.bin"),
            &out,
        )
        .unwrap()
        .unwrap();
    assert_eq!(dl.state, rdm::models::DownloadState::Completed);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn pause_preserves_chunks_and_resume_continues() {
    let _ = tracing_subscriber::fmt().with_max_level(tracing::Level::TRACE).try_init();
    let data = Arc::new(payload(4 * 1024 * 1024));
    let server = TestServer::start(
        data.clone(),
        ServerOptions {
            truncate_at: Some(64 * 1024),
            truncate_first: 2,
            ..server_opts()
        },
    )
    .await;
    let dir = test_dir("pause");
    let out = dir.join("file.bin");
    let data_dir = dir.join("data");

    // Full run completes through retries; then force a second, fresh run that
    // starts from the DB state (simulates `rdm resume`).
    let url = server.url("/file.bin");
    let storage = rdm::storage::database::Storage::open(&data_dir.join("metadata.db")).unwrap();
    let _ = storage;
    let outcome1 = run_engine(url.clone(), out.clone(), data_dir.clone(), |_| {}).await.unwrap();
    assert_eq!(outcome1.state, rdm::models::DownloadState::Completed);

    // Reset state to paused, then resume (reuses completed chunk data).
    let storage = rdm::storage::database::Storage::open(&data_dir.join("metadata.db")).unwrap();
    let dl = storage
        .find_download_by_url_output(&url, &out)
        .unwrap()
        .unwrap();
    storage
        .update_state(dl.id, rdm::models::DownloadState::Paused, None)
        .unwrap();
    let outcome2 = run_engine(url.clone(), out.clone(), data_dir.clone(), |o| {
        o.resume = true;
        o.force = false;
    })
    .await
    .unwrap();
    assert_eq!(outcome2.state, rdm::models::DownloadState::Completed);
    let got = std::fs::read(&out).unwrap();
    assert_eq!(got, *data);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn single_stream_fallback_when_server_has_no_ranges() {
    let _ = tracing_subscriber::fmt().with_max_level(tracing::Level::TRACE).try_init();
    let data = Arc::new(payload(512 * 1024));
    let server = TestServer::start(
        data.clone(),
        ServerOptions {
            no_range: true,
            ..server_opts()
        },
    )
    .await;
    let dir = test_dir("norange");
    let out = dir.join("file.bin");
    let data_dir = dir.join("data");
    let outcome = run_engine(server.url("/file.bin"), out.clone(), data_dir.clone(), |_| {}).await.unwrap();
    assert_eq!(outcome.state, rdm::models::DownloadState::Completed);
    let got = std::fs::read(&out).unwrap();
    assert_eq!(got, *data);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn checksum_verification() {
    let data = Arc::new(payload(1024 * 1024));
    let server = TestServer::start(
        data.clone(),
        server_opts(),
    )
    .await;
    let dir = test_dir("checksum");
    let out = dir.join("file.bin");
    let data_dir = dir.join("data");
    let expected = sha256_of(&data);
    let outcome = run_engine(server.url("/file.bin"), out.clone(), data_dir.clone(), |o| {
        o.checksum = Some(("sha256".into(), expected.clone()));
    })
    .await
    .unwrap();
    assert_eq!(outcome.state, rdm::models::DownloadState::Completed);

    // Wrong checksum must fail.
    let data_dir2 = dir.join("data2");
    let bad = run_engine(
        server.url("/file.bin"),
        dir.join("bad.bin"),
        data_dir2,
        |o| {
            o.checksum = Some(("sha256".into(), "00".repeat(32)));
        },
    )
    .await;
    assert!(bad.is_err(), "wrong checksum must fail the session");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn zero_byte_file() {
    let data = Arc::new(Vec::new());
    let server = TestServer::start(
        data,
        server_opts(),
    )
    .await;
    // The probe path returns 416 for a ranged GET of an empty resource; the
    // engine must handle it as a zero-byte download.
    let dir = test_dir("zero");
    let out = dir.join("empty.bin");
    let data_dir = dir.join("data");
    let outcome = run_engine(server.url("/empty.bin"), out.clone(), data_dir.clone(), |_| {}).await.unwrap();
    assert_eq!(outcome.state, rdm::models::DownloadState::Completed);
    assert_eq!(outcome.bytes, 0);
    assert!(out.exists());
    assert_eq!(std::fs::metadata(&out).unwrap().len(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Regression test for the pause/resume corruption: a chunk resumed at
/// `offset > 0` must APPEND to its chunk file, not overwrite it from
/// position 0, and the resumed session must re-schedule chunks that were
/// left `active` in the database by the pause.
#[tokio::test]
async fn pause_resume_mid_transfer_preserves_content() {
    let data = Arc::new(payload(16 * 1024 * 1024));
    let server = TestServer::start(
        data.clone(),
        ServerOptions {
            truncate_at: None,
            truncate_first: 0,
            no_range: false,
            pace: Some(Duration::from_millis(10)),
            stall: None,
            requests: Arc::new(Mutex::new(0)),
        },
    )
    .await;
    let dir = test_dir("pausemid");
    let out = dir.join("file.bin");
    let data_dir = dir.join("data");
    let url = server.url("/file.bin");

    // One connection => a single 16 MiB chunk; the paced server keeps the
    // transfer in flight long enough to pause it mid-stream.
    let first = tokio::spawn(run_engine(url.clone(), out.clone(), data_dir.clone(), |o| {
        o.connections = 1;
    }));

    // Wait for durable progress, then pause through the control channel —
    // exactly what the GUI's Pause button does.
    let storage = rdm::storage::database::Storage::open(&data_dir.join("metadata.db")).unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let id = loop {
        if let Ok(Some(row)) = storage.find_download_by_url_output(&url, &out) {
            break row.id;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "download row never appeared"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    loop {
        let durable: i64 = storage
            .get_chunks(id)
            .unwrap()
            .iter()
            .map(|c| c.downloaded)
            .sum();
        if durable > 1024 * 1024 {
            storage
                .update_state(id, rdm::models::DownloadState::Paused, None)
                .unwrap();
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "no durable progress");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let outcome1 = tokio::time::timeout(Duration::from_secs(60), first)
        .await
        .expect("paused run timed out")
        .expect("first run join failed")
        .expect("first run failed");
    assert_eq!(outcome1.state, rdm::models::DownloadState::Paused);

    // Every partial chunk file must be a byte-exact slice of the payload.
    for c in storage.get_chunks(id).unwrap() {
        if c.downloaded > 0 {
            let buf = std::fs::read(&c.file_path).unwrap();
            assert_eq!(buf.len() as i64, c.downloaded, "chunk file length");
            let start = c.start as usize;
            assert_eq!(
                &buf[..],
                &data.as_slice()[start..start + buf.len()],
                "partial chunk must match the payload"
            );
        }
    }

    // Resume with 8 connections: the >4 MiB remainder gets split dynamically
    // (children are appended out of idx order) and must still assemble
    // byte-for-byte.
    let outcome2 = tokio::time::timeout(
        Duration::from_secs(60),
        run_engine(url.clone(), out.clone(), data_dir.clone(), |o| {
            o.resume = true;
            o.connections = 8;
        }),
    )
    .await
    .expect("resume timed out")
    .expect("resume failed");
    assert_eq!(outcome2.state, rdm::models::DownloadState::Completed);
    let got = std::fs::read(&out).unwrap();
    assert_eq!(got.len(), data.len());
    assert_eq!(got, *data, "resumed download must match byte-for-byte");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Regression test for merge order after a dynamic split: split children sit
/// at the END of the chunk table while their byte range is mid-file, so the
/// final assembly must order chunks by range, not by table position.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn split_chunks_merge_in_byte_order() {
    let data = Arc::new(payload(15 * 1024 * 1024));
    let server = TestServer::start(
        data.clone(),
        ServerOptions {
            // Hold the first-chunk headers until the other two ranges finish
            // (see `stall`); no pace — siblings should complete ASAP.
            stall: Some((0, Duration::from_secs(15))),
            ..server_opts()
        },
    )
    .await;
    let dir = test_dir("splitorder");
    let out = dir.join("file.bin");
    let data_dir = dir.join("data");

    let outcome = run_engine(server.url("/file.bin"), out.clone(), data_dir.clone(), |o| {
        o.connections = 3;
        // Wider split window than the default 4 MiB remainder threshold.
        o.chunk_size = 256 * 1024;
    })
    .await
    .unwrap();
    assert_eq!(outcome.state, rdm::models::DownloadState::Completed);

    let got = std::fs::read(&out).unwrap();
    assert_eq!(got.len(), data.len());
    assert_eq!(got, *data, "split download must match byte-for-byte");

    // Sanity: a dynamic split really happened (3 planned + >=1 child row).
    let storage = rdm::storage::database::Storage::open(&data_dir.join("metadata.db")).unwrap();
    let dl = storage
        .find_download_by_url_output(&server.url("/file.bin"), &out)
        .unwrap()
        .unwrap();
    let rows = storage.get_chunks(dl.id).unwrap();
    assert!(
        rows.len() >= 4,
        "expected a dynamic split to have happened, found {} chunk rows",
        rows.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
