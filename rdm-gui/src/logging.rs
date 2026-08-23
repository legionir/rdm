//! In-process log capture.
//!
//! The CLI prints `tracing` output to stderr and exposes `-v/-vv/-vvv`. A
//! windowed program has no stderr worth reading, so the same events are routed
//! into a ring buffer that the **App log** tab renders, and the verbosity can
//! be changed at runtime through a `reload` layer.

use std::collections::VecDeque;
use std::io;
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

/// Shared, bounded buffer of formatted log lines.
#[derive(Clone, Default)]
pub struct LogBuffer {
    lines: Arc<Mutex<VecDeque<CapturedLine>>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        LogBuffer::default()
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

/// Writer handed to the `fmt` layer; appends whole lines to the buffer.
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

    /// `directive` is one of [`LEVELS`] (or any `RUST_LOG` expression).
    pub fn set_level(&self, directive: &str) -> Result<(), String> {
        let filter = EnvFilter::try_new(directive).map_err(|e| e.to_string())?;
        self.handle.reload(filter).map_err(|e| e.to_string())
    }
}

/// Install the capturing subscriber. Returns `None` if one is already set
/// (which only happens if the process installed a global subscriber before).
pub fn install(default_level: &str) -> Option<LogControl> {
    let buffer = LogBuffer::new();
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
