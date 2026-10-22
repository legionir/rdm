//! Per-user browser-integration IPC.
//!
//! The browser talks to `rdm-native-host` using the standard native-messaging
//! stdio protocol.  The host validates and atomically queues requests here;
//! the running GUI drains them and opens the appropriate window.  A tiny
//! per-process heartbeat lets the host decide whether it should also launch
//! the GUI.  The queue is deliberately independent of `--data-dir`, so custom
//! metadata locations and multiple GUI invocations still use the same bridge.

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);
const HEARTBEAT_TTL: Duration = Duration::from_secs(15);
const MAX_URL_BYTES: usize = 64 * 1024;
const MAX_LINKS: usize = 5_000;
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// One anchor discovered on a web page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserLink {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Requests accepted from the Chrome/Firefox extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BrowserRequest {
    /// Open the existing New download form with one URL.
    OpenNew { url: String },
    /// Open the checkbox list and add only the links accepted by the user.
    SelectLinks { links: Vec<BrowserLink> },
}

impl BrowserRequest {
    /// Validate URLs, trim labels, and de-duplicate batch links while keeping
    /// page order.  Both the native host and the GUI apply this boundary check.
    pub fn normalized(self) -> Result<Self, String> {
        match self {
            BrowserRequest::OpenNew { url } => Ok(BrowserRequest::OpenNew {
                url: normalize_url(&url)?,
            }),
            BrowserRequest::SelectLinks { links } => {
                if links.len() > MAX_LINKS {
                    return Err(format!("too many links (maximum {MAX_LINKS})"));
                }
                let mut seen = HashSet::new();
                let mut normalized = Vec::with_capacity(links.len());
                for link in links {
                    let url = normalize_url(&link.url)?;
                    if !seen.insert(url.clone()) {
                        continue;
                    }
                    let label = link.label.and_then(|label| {
                        let label = label.trim();
                        if label.is_empty() {
                            None
                        } else {
                            // Anchor text is only a display hint.  Keep it
                            // bounded even on a generated/very large page.
                            Some(label.chars().take(300).collect())
                        }
                    });
                    normalized.push(BrowserLink { url, label });
                }
                if normalized.is_empty() {
                    return Err("the page contains no downloadable links".to_string());
                }
                Ok(BrowserRequest::SelectLinks { links: normalized })
            }
        }
    }
}

fn normalize_url(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("empty download URL".to_string());
    }
    if value.len() > MAX_URL_BYTES {
        return Err(format!("download URL exceeds {MAX_URL_BYTES} bytes"));
    }
    let parsed = url::Url::parse(value).map_err(|err| format!("invalid download URL: {err}"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("only absolute HTTP/HTTPS download links are supported".to_string());
    }
    Ok(value.to_string())
}

/// Queue one browser request in the per-user bridge directory.
pub fn enqueue(request: BrowserRequest) -> Result<PathBuf, String> {
    enqueue_in(&default_bridge_dir(), request)
}

/// Is a GUI instance heartbeating in the current user's bridge?
pub fn has_running_instance() -> bool {
    has_running_instance_in(&default_bridge_dir())
}

/// GUI-side bridge owner.  Polling is cheap: only directory metadata and new
/// request files are touched, and the heartbeat is written every two seconds.
pub struct BrowserBridge {
    root: PathBuf,
    heartbeat_path: PathBuf,
    last_heartbeat: Instant,
}

impl BrowserBridge {
    pub fn new() -> io::Result<Self> {
        Self::with_root(default_bridge_dir())
    }

    pub fn with_root(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(root.join("requests"))?;
        fs::create_dir_all(root.join("instances"))?;
        let heartbeat_path = root
            .join("instances")
            .join(format!("{}.heartbeat", std::process::id()));
        let mut bridge = BrowserBridge {
            root,
            heartbeat_path,
            last_heartbeat: Instant::now(),
        };
        bridge.heartbeat()?;
        Ok(bridge)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Drain queued requests in filename (therefore arrival) order.  Invalid
    /// requests are removed and returned as errors so the GUI can report them
    /// without blocking later valid requests.
    pub fn poll(&mut self) -> io::Result<Vec<Result<BrowserRequest, String>>> {
        if self.last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            self.heartbeat()?;
        }
        drain_requests(&self.root)
    }

    fn heartbeat(&mut self) -> io::Result<()> {
        fs::write(&self.heartbeat_path, std::process::id().to_string())?;
        self.last_heartbeat = Instant::now();
        Ok(())
    }
}

impl Drop for BrowserBridge {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.heartbeat_path);
    }
}

/// Per-user, persistent location shared by the host and every GUI invocation.
pub fn default_bridge_dir() -> PathBuf {
    user_state_dir().join("RDM").join("browser")
}

#[cfg(target_os = "windows")]
fn user_state_dir() -> PathBuf {
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        if !base.is_empty() {
            return PathBuf::from(base);
        }
    }
    fallback_state_dir()
}

#[cfg(target_os = "macos")]
fn user_state_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support");
        }
    }
    fallback_state_dir()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn user_state_dir() -> PathBuf {
    if let Some(base) = std::env::var_os("XDG_STATE_HOME") {
        if !base.is_empty() {
            return PathBuf::from(base);
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home).join(".local").join("state");
        }
    }
    fallback_state_dir()
}

#[allow(dead_code)]
fn fallback_state_dir() -> PathBuf {
    // This is only a fallback for unusually incomplete process environments.
    // Include the account name where available so a shared /tmp is not shared
    // between users.
    let account = std::env::var_os("USER")
        .or_else(|| std::env::var_os("USERNAME"))
        .unwrap_or_else(|| "unknown".into());
    std::env::temp_dir().join("rdm").join(account)
}

fn enqueue_in(root: &Path, request: BrowserRequest) -> Result<PathBuf, String> {
    let request = request.normalized()?;
    let requests = root.join("requests");
    fs::create_dir_all(&requests)
        .map_err(|err| format!("cannot create browser request queue: {err}"))?;
    let json = serde_json::to_vec(&request)
        .map_err(|err| format!("cannot encode browser request: {err}"))?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "{stamp:032}-{:010}-{sequence:010}",
        std::process::id()
    );
    let temporary = requests.join(format!("{stem}.tmp"));
    let destination = requests.join(format!("{stem}.json"));

    let result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&json)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &destination)?;
        Ok(())
    })();
    if let Err(err) = result {
        let _ = fs::remove_file(&temporary);
        return Err(format!("cannot queue browser request: {err}"));
    }
    Ok(destination)
}

fn drain_requests(root: &Path) -> io::Result<Vec<Result<BrowserRequest, String>>> {
    let requests = root.join("requests");
    let mut paths = Vec::new();
    for entry in fs::read_dir(requests)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "json") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut output = Vec::with_capacity(paths.len());
    for path in paths {
        match fs::read(&path) {
            Err(err) => output.push(Err(format!(
                "cannot read browser request {}: {err}",
                path.display()
            ))),
            Ok(bytes) => {
                let request = serde_json::from_slice::<BrowserRequest>(&bytes)
                    .map_err(|err| format!("invalid browser request: {err}"))
                    .and_then(BrowserRequest::normalized);
                // Reading succeeded, so this file will not be retried forever:
                // malformed input is reported and discarded, valid input is
                // handed to the GUI exactly once.
                let _ = fs::remove_file(&path);
                output.push(request);
            }
        }
    }
    Ok(output)
}

fn has_running_instance_in(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root.join("instances")) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "heartbeat") {
            return false;
        }
        entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .map(|modified| {
                modified
                    .elapsed()
                    .map(|age| age <= HEARTBEAT_TTL)
                    .unwrap_or(true)
            })
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rdm-browser-test-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn validates_and_deduplicates_links() {
        let request = BrowserRequest::SelectLinks {
            links: vec![
                BrowserLink {
                    url: " https://example.com/a.zip ".into(),
                    label: Some("  A release  ".into()),
                },
                BrowserLink {
                    url: "https://example.com/a.zip".into(),
                    label: Some("duplicate".into()),
                },
                BrowserLink {
                    url: "https://example.com/b.zip".into(),
                    label: None,
                },
            ],
        }
        .normalized()
        .unwrap();
        let BrowserRequest::SelectLinks { links } = request else {
            panic!("wrong request kind");
        };
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].url, "https://example.com/a.zip");
        assert_eq!(links[0].label.as_deref(), Some("A release"));
        assert!(BrowserRequest::OpenNew {
            url: "file:///etc/passwd".into()
        }
        .normalized()
        .is_err());
    }

    #[test]
    fn queued_requests_round_trip_once_and_heartbeat_is_visible() {
        let root = test_root("round-trip");
        let mut bridge = BrowserBridge::with_root(root.clone()).unwrap();
        assert!(has_running_instance_in(&root));
        let request = BrowserRequest::OpenNew {
            url: "https://example.com/file.zip".into(),
        };
        enqueue_in(&root, request.clone()).unwrap();

        let drained = bridge.poll().unwrap();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained.into_iter().next().unwrap().unwrap(), request);
        assert!(bridge.poll().unwrap().is_empty());
        drop(bridge);
        assert!(!has_running_instance_in(&root));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_request_does_not_block_the_next_one() {
        let root = test_root("malformed");
        let requests = root.join("requests");
        fs::create_dir_all(&requests).unwrap();
        fs::write(requests.join("000.json"), b"not json").unwrap();
        let mut bridge = BrowserBridge::with_root(root.clone()).unwrap();
        enqueue_in(
            &root,
            BrowserRequest::OpenNew {
                url: "https://example.com/after.zip".into(),
            },
        )
        .unwrap();
        let drained = bridge.poll().unwrap();
        assert_eq!(drained.len(), 2);
        assert!(drained[0].is_err());
        assert!(drained[1].is_ok());
        let _ = fs::remove_dir_all(root);
    }
}
