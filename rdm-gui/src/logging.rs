//! In-process log capture.
//!
//! The CLI prints `tracing` output to stderr and exposes `-v/-vv/-vvv`. A
//! windowed program has no stderr worth reading, so the same events are routed
//! into a ring buffer that the **App log** tab renders, and the verbosity can
//! be changed at runtime through a `reload` layer.
//!
//! The same lines are appended to `rdm-gui.log` in the data directory. That is
//! not decoration: when a user reports “the tray froze” (round 3), the window
//! they would have to read the App log in is exactly the window the freeze has
//! taken away from them — the file is what makes such a report diagnosable
//! afterwards, including the lines the tray relays write from their own
//! threads. It is one line at a time, flushed as it is written, so an abrupt
//! exit (`std::process::exit`) cannot swallow it.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{reload, EnvFilter};

const CAPACITY: usize = 2000;

/// Levels offered by the settings combo, in `EnvFilter` syntax.
pub const LEVELS: [&str; 6] = ["off", "error", "warn", "info", "debug", "trace"];

/// A captured log record.
#[derive(Debug, Clone)]
pub struct CapturedLine {
    pub level: &'static str,
    pub text: String,
}

/// Shared, bounded buffer of formatted log lines (and the file beside it).
#[derive(Clone, Default)]
pub struct LogBuffer {
    lines: Arc<Mutex<VecDeque<CapturedLine>>>,
    file: LogFile,
}

impl LogBuffer {
    pub fn new() -> Self {
        LogBuffer::default()
    }

    /// The same buffer, writing every line to `file` as well.
    pub fn with_file(file: LogFile) -> Self {
        LogBuffer {
            file,
            ..LogBuffer::default()
        }
    }

    fn push(&self, raw: &str) {
        let raw = raw.trim_end();
        if raw.is_empty() {
            return;
        }
        let (level, text) = split_level(raw);
        let mut guard = match self.lines.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        if guard.len() >= CAPACITY {
            guard.pop_front();
        }
        guard.push_back(CapturedLine {
            level,
            text: text.to_string(),
        });
        // The file gets the same line here, the one place every line passes
        // through: the window's App log is gone exactly when it is needed most.
        self.file.append(raw);
    }

    /// Take everything captured since the last call.
    pub fn drain(&self) -> Vec<CapturedLine> {
        let mut guard = match self.lines.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.drain(..).collect()
    }
}

/// `INFO probing https://… — ok` → `("info", "probing https://… — ok")`
fn split_level(line: &str) -> (&'static str, &str) {
    let trimmed = line.trim_start();
    for (needle, level) in [
        ("ERROR", "error"),
        ("WARN", "warn"),
        ("INFO", "info"),
        ("DEBUG", "debug"),
        ("TRACE", "trace"),
    ] {
        if let Some(rest) = trimmed.strip_prefix(needle) {
            return (level, rest.trim_start());
        }
    }
    ("info", trimmed)
}

/// The log file next to the metadata database, when one can be opened.
///
/// A log that cannot be written is not worth an error dialog: the app keeps the
/// in-window log and carries on.
#[derive(Clone, Default)]
pub struct LogFile {
    path: Option<Arc<PathBuf>>,
}

impl LogFile {
    /// Open (append) `dir/rdm-gui.log`. `None` disables the file.
    pub fn open(dir: Option<&Path>) -> Self {
        let path = dir.map(|dir| dir.join("rdm-gui.log"));
        if let Some(path) = &path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if File::options().create(true).append(true).open(path).is_err() {
                return Self { path: None };
            }
        }
        Self {
            path: path.map(Arc::new),
        }
    }

    /// Where the file is, so the app can say so once at start-up.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref().map(PathBuf::as_path)
    }

    fn append(&self, line: &str) {
        let Some(path) = &self.path else {
            return;
        };
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path.as_ref()) {
            // One line per open, closed immediately: the file stays readable
            // while the app runs, and a hard exit loses nothing already written.
            let _ = writeln!(file, "{line}");
        }
    }
}

/// Writer handed to the `fmt` layer; appends whole lines to the buffer (which
/// passes each one on to the log file).
pub struct BufferWriter {
    buffer: LogBuffer,
}

impl io::Write for BufferWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        for line in text.lines() {
            self.buffer.push(line);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for LogBuffer {
    type Writer = BufferWriter;

    fn make_writer(&'a self) -> Self::Writer {
        BufferWriter {
            buffer: self.clone(),
        }
    }
}

/// Lets the settings panel change verbosity without a restart.
#[derive(Clone)]
pub struct LogControl {
    handle: reload::Handle<EnvFilter, tracing_subscriber::Registry>,
    buffer: LogBuffer,
}

impl LogControl {
    pub fn buffer(&self) -> &LogBuffer {
        &self.buffer
    }

    /// Where the log file is, when one is being written.
    pub fn log_file(&self) -> Option<&Path> {
        // Borrowed from the live buffer, not from a clone of the handle.
        self.buffer.file.path.as_deref().map(PathBuf::as_path)
    }

    /// `directive` is one of [`LEVELS`] (or any `RUST_LOG` expression).
    pub fn set_level(&self, directive: &str) -> Result<(), String> {
        let filter = EnvFilter::try_new(directive).map_err(|e| e.to_string())?;
        self.handle.reload(filter).map_err(|e| e.to_string())
    }
}

/// Install the capturing subscriber. Returns `None` if one is already set
/// (which only happens if the process installed a global subscriber before).
pub fn install(default_level: &str, data_dir: Option<&Path>) -> Option<LogControl> {
    let buffer = LogBuffer::with_file(LogFile::open(data_dir));
    let initial = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => EnvFilter::try_new(default_level).unwrap_or_else(|_| EnvFilter::new("info")),
    };
    let (filter, handle) = reload::Layer::new(initial);
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_target(false)
        .without_time()
        .with_writer(buffer.clone());
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init()
        .ok()?;
    Some(LogControl { handle, buffer })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_the_level_off_a_line() {
        assert_eq!(split_level("INFO probing https://x — ok"), ("info", "probing https://x — ok"));
        assert_eq!(split_level(" WARN  slow"), ("warn", "slow"));
        assert_eq!(split_level("no level here"), ("info", "no level here"));
    }

    #[test]
    fn every_line_also_lands_in_the_log_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let buffer = LogBuffer::with_file(LogFile::open(Some(dir.path())));
        buffer.push("INFO first");
        buffer.push("WARN second");
        // Read through the same public surface the app uses for the setting.
        let written = std::fs::read_to_string(dir.path().join("rdm-gui.log")).expect("log file");
        assert!(written.contains("INFO first"), "log file: {written:?}");
        assert!(written.contains("WARN second"), "log file: {written:?}");
        assert_eq!(buffer.drain().len(), 2, "the window keeps its lines too");
    }

    #[test]
    fn a_missing_data_dir_disables_the_file_without_failing() {
        // No directory to write to: the app must keep working, not refuse to
        // start over a log file.
        let buffer = LogBuffer::with_file(LogFile::open(None));
        assert!(buffer.file.path.is_none());
        buffer.push("INFO still captured in the window");
        assert_eq!(buffer.drain().len(), 1);
    }

    #[test]
    fn buffer_keeps_lines_in_order_and_drains() {
        let buffer = LogBuffer::new();
        buffer.push("INFO first");
        buffer.push("ERROR second");
        buffer.push("   ");
        let drained = buffer.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].level, "info");
        assert_eq!(drained[0].text, "first");
        assert_eq!(drained[1].level, "error");
        assert!(buffer.drain().is_empty());
    }
}
