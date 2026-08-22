//! End-to-end tests: a local HTTP server with Range support exercises the
//! engine (segmented download, resume after premature disconnect, single
//! stream fallback, checksum verification).

use std::io::Write;
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

struct ServerOptions {
    /// For the first `truncate_first` requests, close after this many bytes.
    truncate_at: Option<usize>,
    /// Number of requests to apply `truncate_at` to.
    truncate_first: usize,
    /// If true, the server does not advertise/support ranges (always 200).
    no_range: bool,
    /// Number of requests handled so far.
    requests: Arc<Mutex<usize>>,
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
                let opts = ServerOptions {
                    truncate_at: opts.truncate_at,
                    truncate_first: opts.truncate_first,
                    no_range: opts.no_range,
                    requests: opts.requests.clone(),
                };
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
            (Some((s, e)), false) if s >= data.len() as u64 => {
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
            return Ok(());
        }
    }
    let mut written = 0usize;
    while written < slice.len() {
        let n = (slice.len() - written).min(64 * 1024);
        sock.write_all(&slice[written..written + n]).await?;
        written += n;
    }
    Ok(())
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
        ServerOptions {
            truncate_at: None,
            truncate_first: 0,
            no_range: false,
            requests: Arc::new(Mutex::new(0)),
        },
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
            no_range: false,
            requests: Arc::new(Mutex::new(0)),
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
            no_range: false,
            requests: Arc::new(Mutex::new(0)),
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
            truncate_at: None,
            truncate_first: 0,
            no_range: true,
            requests: Arc::new(Mutex::new(0)),
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
        ServerOptions {
            truncate_at: None,
            truncate_first: 0,
            no_range: false,
            requests: Arc::new(Mutex::new(0)),
        },
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
        ServerOptions {
            truncate_at: None,
            truncate_first: 0,
            no_range: false,
            requests: Arc::new(Mutex::new(0)),
        },
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
