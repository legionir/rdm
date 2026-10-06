//! Links that arrive from outside the app's own UI.
//!
//! Three sources feed the same queue, which the app drains once per frame:
//!
//! * the tray menu (“New download”) and its clipboard shortcut;
//! * the floating drop target (“floating bottom”);
//! * files or text dropped on the main window.
//!
//! Parsing lives here, separate from the OS plumbing, so the interesting rules
//! — what counts as a droppable link, what a `.url`/`.txt` link file is, what
//! to say when the payload is not a link — are unit-tested without a window.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::clipboard;

/// Files larger than this are never read as “a file that contains a link”.
const MAX_LINK_FILE_BYTES: u64 = 64 * 1024;

/// A queue of links waiting for the next frame.
///
/// Cloned handles share the same queue; `Arc<Mutex<…>>` keeps every producer
/// (tray menu, drop windows, tests) decoupled from the app's frame loop.
#[derive(Clone, Debug, Default)]
pub struct ImportBus {
    inner: Arc<Mutex<Vec<String>>>,
}

impl ImportBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue one link (ignored when it is not a link).
    pub fn push(&self, url: impl Into<String>) -> bool {
        let url = url.into();
        if !clipboard::looks_like_url(&url) {
            return false;
        }
        if let Ok(mut queue) = self.inner.lock() {
            queue.push(url);
            return true;
        }
        false
    }

    /// Take everything that arrived since the last call.
    pub fn drain(&self) -> Vec<String> {
        self.inner
            .lock()
            .map(|mut queue| std::mem::take(&mut *queue))
            .unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.inner
            .lock()
            .map(|queue| queue.is_empty())
            .unwrap_or(true)
    }
}

/// What a dropped payload turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropReport {
    /// The link, when one was found.
    pub url: Option<String>,
    /// One sentence for the status bar — always set, so a drop is never silent.
    pub note: String,
}

impl DropReport {
    fn found(url: String) -> Self {
        Self {
            url: Some(url.clone()),
            note: format!("Dropped link accepted: {url}"),
        }
    }

    fn ignored(note: impl Into<String>) -> Self {
        Self {
            url: None,
            note: note.into(),
        }
    }
}

/// Interpret a drop payload: browsers put the link in the plain text, file
/// managers put paths in the URI list.
pub fn interpret_drop(text: Option<&str>, uri_list: Option<&str>) -> DropReport {
    if let Some(text) = text {
        if let Some(url) = url_from_text(text) {
            return DropReport::found(url);
        }
        // A dropped `.url`/`.txt` file arrives as its own path.
        if let Some(path) = path_from_text(text) {
            return match url_from_file(&path) {
                Some(url) => DropReport::found(url),
                None => DropReport::ignored(format!(
                    "“{}” does not contain a link — drop the link itself",
                    path.display()
                )),
            };
        }
    }

    if let Some(list) = uri_list {
        let paths = parse_uri_list(list);
        for path in &paths {
            if let Some(url) = url_from_file(path) {
                return DropReport::found(url);
            }
        }
        if let Some(first) = paths.first() {
            return DropReport::ignored(format!(
                "“{}” does not contain a link — drop the link itself",
                first.display()
            ));
        }
    }

    DropReport::ignored("Nothing was dropped that looks like a link.")
}

/// The link inside a piece of text, if it is a link and nothing else.
pub fn url_from_text(text: &str) -> Option<String> {
    let text = text.trim();
    if clipboard::looks_like_url(text) {
        return Some(text.to_string());
    }
    None
}

/// Treat a piece of text as a local path when it plausibly is one.
fn path_from_text(text: &str) -> Option<PathBuf> {
    let text = text.trim();
    if text.is_empty() || text.contains('\n') {
        return None;
    }
    let path = PathBuf::from(text);
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

/// A link from a local file: either the file is itself a URL/`.url`/text file
/// holding one, or it is not a link at all.
pub fn url_from_file(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_LINK_FILE_BYTES {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    first_url_in(&content)
}

/// The first link in `content`, understanding the `URL=` form written by
/// Windows `.url` shortcut files as well as a bare link or a list of them.
pub fn first_url_in(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('[') || line.starts_with('#') {
            continue;
        }
        let candidate = match line.split_once('=') {
            Some((key, value)) if key.trim().eq_ignore_ascii_case("url") => value.trim(),
            _ => line,
        };
        if let Some(url) = url_from_text(candidate) {
            return Some(url);
        }
    }
    None
}

/// `file:///C:/x/y.txt` lines (the `text/uri-list` format) → local paths.
pub fn parse_uri_list(list: &str) -> Vec<PathBuf> {
    list.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let rest = line.strip_prefix("file://")?;
            // `file:///C:/x` → `/C:/x`; `file://host/share` keeps the host path.
            let rest = rest.strip_prefix("localhost").unwrap_or(rest);
            let decoded = percent_decode(rest);
            let trimmed = if cfg!(windows) {
                decoded
                    .strip_prefix('/')
                    .map(|s| s.replace('/', "\\"))
                    .unwrap_or(decoded)
            } else {
                decoded
            };
            Some(PathBuf::from(trimmed))
        })
        .collect()
}

/// Minimal percent-decoding for dropped paths (`%20` → space).
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(byte) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bus_only_carries_links_and_drains_once() {
        let bus = ImportBus::new();
        assert!(bus.is_empty());
        assert!(bus.push("https://example.com/a.zip"));
        assert!(!bus.push("not a link"));
        assert_eq!(bus.drain(), vec!["https://example.com/a.zip".to_string()]);
        assert!(bus.is_empty(), "drain takes everything");
        assert!(bus.drain().is_empty());
    }

    #[test]
    fn cloned_buses_share_one_queue() {
        let bus = ImportBus::new();
        let other = bus.clone();
        other.push("https://example.com/b.zip");
        assert_eq!(bus.drain().len(), 1);
    }

    #[test]
    fn a_dropped_link_is_taken_from_the_plain_text() {
        let report = interpret_drop(Some("https://example.com/file.zip"), None);
        assert_eq!(report.url.as_deref(), Some("https://example.com/file.zip"));
        assert!(report.note.contains("accepted"));
    }

    #[test]
    fn a_drop_that_is_not_a_link_says_so_instead_of_staying_silent() {
        for payload in [
            interpret_drop(Some("just some text"), None),
            interpret_drop(None, None),
            interpret_drop(Some(r"C:\downloads\notes.rtf"), None),
        ] {
            assert!(payload.url.is_none());
            assert!(!payload.note.is_empty(), "every drop produces a sentence");
        }
    }

    #[test]
    fn a_txt_file_holding_a_link_works() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("link.txt");
        std::fs::write(&file, "https://example.com/from-file.bin\n").unwrap();
        let report = interpret_drop(Some(&file.display().to_string()), None);
        assert_eq!(
            report.url.as_deref(),
            Some("https://example.com/from-file.bin")
        );
    }

    #[test]
    fn a_windows_url_shortcut_works() {
        let content = "[InternetShortcut]\r\nURL=https://example.com/shortcut.zip\r\nIconIndex=0\r\n";
        assert_eq!(
            first_url_in(content).as_deref(),
            Some("https://example.com/shortcut.zip")
        );
    }

    #[test]
    fn a_file_without_a_link_is_reported_as_such() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "shopping list\nmilk\n").unwrap();
        let report = interpret_drop(Some(&file.display().to_string()), None);
        assert!(report.url.is_none());
        assert!(report.note.contains("does not contain a link"), "{}", report.note);
    }

    #[test]
    fn oversized_files_are_not_read() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("huge.txt");
        std::fs::write(&file, "x".repeat((MAX_LINK_FILE_BYTES + 1) as usize)).unwrap();
        assert!(url_from_file(&file).is_none());
    }

    #[test]
    fn uri_lists_become_paths_and_percent_decoding_holds() {
        let paths = parse_uri_list("# comment\nfile:///tmp/a%20b.txt\n\n");
        assert_eq!(paths.len(), 1);
        assert!(paths[0].to_string_lossy().contains("a b.txt"));
    }

    #[test]
    fn a_uri_list_drop_finds_the_link_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("dropped.txt");
        std::fs::write(&file, "https://example.com/uri-list.bin").unwrap();
        let uri = format!("file://{}", file.display());
        let report = interpret_drop(None, Some(&uri));
        assert_eq!(report.url.as_deref(), Some("https://example.com/uri-list.bin"));
    }
}
