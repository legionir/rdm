//! Persisted GUI preferences (`<data-dir>/settings.toml`).
//!
//! These are the defaults applied to every new download, i.e. the equivalent
//! of always typing the same `rdm download` flags. The file is watched, so an
//! edit made in a text editor shows up in the window within a second.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::backend::StartRequest;

pub const SETTINGS_FILE: &str = "settings.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettings {
    /// Directory holding `metadata.db` (the CLI's `--data-dir`).
    pub data_dir: String,
    /// Where finished files land when the Add form leaves "Output" empty.
    pub download_dir: String,
    pub connections: u16,
    pub retries: u32,
    pub chunk_size: String,
    pub max_speed: String,
    pub user_agent: String,
    pub timeout_secs: u64,
    /// How many transfers may run at the same time (0 = unlimited).
    pub max_concurrent: u32,
    /// How often the download table is re-read from SQLite.
    pub refresh_ms: u64,
    /// Ask before removing a record.
    pub confirm_remove: bool,
    /// Delete the assembled file too when removing.
    pub purge_on_remove: bool,
    pub dark_mode: bool,
    /// Verbosity of the captured engine log (`off`..`trace`), like `-v/-vv/-vvv`.
    pub log_level: String,
    /// Pre-fill the URL field from the clipboard when it holds a link.
    pub clipboard_prefill: bool,
    /// Closing the window hides it into the tray instead of quitting.
    pub close_to_tray: bool,
    /// Show the floating drop target above the taskbar clock area.
    pub drop_target_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            data_dir: ".rdm".to_string(),
            download_dir: String::new(),
            connections: 8,
            retries: 5,
            chunk_size: "1MiB".to_string(),
            max_speed: String::new(),
            user_agent: String::new(),
            timeout_secs: 30,
            max_concurrent: 3,
            refresh_ms: 600,
            confirm_remove: true,
            purge_on_remove: false,
            dark_mode: true,
            log_level: "info".to_string(),
            clipboard_prefill: true,
            close_to_tray: true,
            drop_target_enabled: false,
        }
    }
}

impl AppSettings {
    /// Defaults for a fresh "New download" form.
    pub fn to_request(&self) -> StartRequest {
        StartRequest {
            url: String::new(),
            output: self.download_dir.clone(),
            connections: self.connections,
            retries: self.retries,
            chunk_size: self.chunk_size.clone(),
            max_speed: self.max_speed.clone(),
            checksum: String::new(),
            user_agent: self.user_agent.clone(),
            timeout_secs: self.timeout_secs,
            resume: false,
            force: false,
        }
    }

    pub fn serialize(&self) -> String {
        let mut out = String::new();
        out.push_str("# rdm-gui settings — edited live by the GUI, safe to hand-edit\n");
        out.push_str(&format!("data_dir = \"{}\"\n", escape(&self.data_dir)));
        out.push_str(&format!(
            "download_dir = \"{}\"\n",
            escape(&self.download_dir)
        ));
        out.push_str(&format!("connections = {}\n", self.connections));
        out.push_str(&format!("retries = {}\n", self.retries));
        out.push_str(&format!("chunk_size = \"{}\"\n", escape(&self.chunk_size)));
        out.push_str(&format!("max_speed = \"{}\"\n", escape(&self.max_speed)));
        out.push_str(&format!("user_agent = \"{}\"\n", escape(&self.user_agent)));
        out.push_str(&format!("timeout_secs = {}\n", self.timeout_secs));
        out.push_str(&format!("max_concurrent = {}\n", self.max_concurrent));
        out.push_str(&format!("refresh_ms = {}\n", self.refresh_ms));
        out.push_str(&format!("confirm_remove = {}\n", self.confirm_remove));
        out.push_str(&format!("purge_on_remove = {}\n", self.purge_on_remove));
        out.push_str(&format!("dark_mode = {}\n", self.dark_mode));
        out.push_str(&format!("log_level = \"{}\"\n", escape(&self.log_level)));
        out.push_str(&format!("clipboard_prefill = {}\n", self.clipboard_prefill));
        out.push_str(&format!("close_to_tray = {}\n", self.close_to_tray));
        out.push_str(&format!(
            "drop_target_enabled = {}\n",
            self.drop_target_enabled
        ));
        out
    }

    pub fn parse(content: &str) -> Self {
        let mut s = AppSettings::default();
        for line in content.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = unescape(value.trim());
            match key {
                "data_dir" => s.data_dir = value.to_string(),
                // `download_dir` is the historical name; keep both spellings.
                "download_dir" | "output_dir" => s.download_dir = value.to_string(),
                "connections" | "max_connections" => {
                    s.connections = value.parse().unwrap_or(s.connections).clamp(1, 128)
                }
                "retries" | "retry_limit" => s.retries = value.parse().unwrap_or(s.retries),
                "chunk_size" => s.chunk_size = value.to_string(),
                "chunk_size_mb" => {
                    if let Ok(mb) = value.parse::<u32>() {
                        s.chunk_size = format!("{mb}MiB");
                    }
                }
                "max_speed" => s.max_speed = value.to_string(),
                "user_agent" => s.user_agent = value.to_string(),
                "timeout_secs" | "timeout" => {
                    s.timeout_secs = value.parse().unwrap_or(s.timeout_secs)
                }
                "max_concurrent" | "max_parallel" => {
                    s.max_concurrent = value.parse().unwrap_or(s.max_concurrent).min(64)
                }
                "refresh_ms" => s.refresh_ms = value.parse::<u64>().unwrap_or(s.refresh_ms).clamp(100, 10_000),
                "confirm_remove" => s.confirm_remove = value == "true",
                "purge_on_remove" => s.purge_on_remove = value == "true",
                "dark_mode" => s.dark_mode = value == "true",
                "log_level" | "verbosity" => {
                    if crate::logging::LEVELS.contains(&value.as_str()) {
                        s.log_level = value;
                    }
                }
                "clipboard_prefill" => s.clipboard_prefill = truthy(&value),
                "close_to_tray" => s.close_to_tray = truthy(&value),
                "drop_target_enabled" => s.drop_target_enabled = truthy(&value),
                _ => {}
            }
        }
        s
    }
}

/// Only characters that break the `key = "value"` form are escaped.
///
/// Backslashes are **not** escaped on purpose: every path in this file is a
/// Windows path (`C:\downloads`), and the previous symmetric
/// `replace('\\', "\\\\")` had no matching decoder, so each save/load cycle
/// doubled every backslash (the reported `C:\\\\download\\\\rdm` bug).
fn escape(value: &str) -> String {
    value.replace('\n', "\\n").replace('\t', "\\t").replace('"', "\\\"")
}

/// The inverse of [`escape`], accepting both quoted and bare values.
fn unescape(value: &str) -> String {
    let value = value.trim();
    let inner = match value.strip_prefix('"') {
        Some(rest) => rest.strip_suffix('"').unwrap_or(rest),
        None => value,
    };
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('"') => out.push('"'),
            // A stray backslash (the normal case for Windows paths) is kept.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn truthy(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "yes" | "1" | "on")
}

/// Loads the settings file and notices external edits (hot reload).
pub struct SettingsStore {
    path: PathBuf,
    last_modified: Option<SystemTime>,
    settings: AppSettings,
    /// `--data-dir` from the command line: it wins over the value saved in the
    /// file, for this load and every hot reload. It is also what the file ends
    /// up recording — the file lives inside that directory, so the recorded
    /// directory is always its own and can never point somewhere else.
    cli_data_dir: Option<String>,
}

impl SettingsStore {
    /// `data_dir` is the directory this run uses; `explicit` says whether it
    /// came from `--data-dir` (then it outranks the saved value).
    pub fn new(data_dir: &Path, explicit: bool) -> Self {
        let mut store = SettingsStore {
            path: data_dir.join(SETTINGS_FILE),
            last_modified: None,
            settings: AppSettings::default(),
            cli_data_dir: None,
        };
        store.settings.data_dir = data_dir.display().to_string();
        if explicit {
            store.cli_data_dir = Some(data_dir.display().to_string());
        }
        store.load();
        store
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut AppSettings {
        &mut self.settings
    }

    pub fn load(&mut self) {
        self.last_modified = modified_at(&self.path);
        if let Ok(content) = std::fs::read_to_string(&self.path) {
            self.settings = AppSettings::parse(&content);
        }
        // A command-line directory always wins, for every later reload too.
        if let Some(cli) = &self.cli_data_dir {
            self.settings.data_dir = cli.clone();
        }
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // The file belongs to the directory it lives in; writing the
        // process-relative default back would silently move the next start.
        if let Some(cli) = &self.cli_data_dir {
            self.settings.data_dir = cli.clone();
        } else if self.settings.data_dir.is_empty() {
            self.settings.data_dir = self
                .path
                .parent()
                .map(|dir| dir.display().to_string())
                .unwrap_or_else(|| self.settings.data_dir.clone());
        }
        std::fs::write(&self.path, self.settings.serialize())?;
        self.last_modified = modified_at(&self.path);
        Ok(())
    }

    /// Returns `true` when the file changed on disk since the last read.
    pub fn poll_external_change(&mut self) -> bool {
        let current = modified_at(&self.path);
        if current.is_some() && current != self.last_modified {
            self.load();
            true
        } else {
            false
        }
    }

    /// Re-target the store after the data dir changed *in the UI*; that choice
    /// becomes the new preference (the CLI override no longer applies).
    pub fn relocate(&mut self, data_dir: &Path) {
        self.path = data_dir.join(SETTINGS_FILE);
        self.last_modified = None;
        self.cli_data_dir = None;
        self.load();
        self.settings.data_dir = data_dir.display().to_string();
    }
}

fn modified_at(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_the_file_format() {
        let mut settings = AppSettings::default();
        settings.download_dir = "/tmp/dl".to_string();
        settings.connections = 16;
        settings.chunk_size = "4MiB".to_string();
        settings.max_speed = "5MB/s".to_string();
        settings.max_concurrent = 2;
        settings.log_level = "debug".to_string();
        settings.confirm_remove = false;
        let parsed = AppSettings::parse(&settings.serialize());
        assert_eq!(parsed, settings);
    }

    #[test]
    fn accepts_the_legacy_key_names() {
        let parsed = AppSettings::parse(
            "output_dir = \"/data\"\nmax_connections = 12\nchunk_size_mb = 8\nretry_limit = 9\n",
        );
        assert_eq!(parsed.download_dir, "/data");
        assert_eq!(parsed.connections, 12);
        assert_eq!(parsed.chunk_size, "8MiB");
        assert_eq!(parsed.retries, 9);
    }

    #[test]
    fn ignores_comments_and_junk_and_clamps() {
        let parsed = AppSettings::parse(
            "# comment\nconnections = 999\nrefresh_ms = 1\nnonsense\nlog_level = \"nope\"\n",
        );
        assert_eq!(parsed.connections, 128);
        assert_eq!(parsed.refresh_ms, 100);
        assert_eq!(parsed.log_level, "info");
    }

    #[test]
    fn windows_paths_survive_any_number_of_round_trips() {
        // The reported bug: `C:\download\rdm` came back as `C:\\download\\rdm`.
        let mut settings = AppSettings::default();
        settings.data_dir = r"C:\download\rdm".to_string();
        settings.download_dir = r"C:\download\rdm\finished".to_string();

        let text = settings.serialize();
        let mut current = AppSettings::parse(&text);
        assert_eq!(current, settings, "first read");
        assert!(
            !text.contains(r"\\"),
            "no doubled backslashes may be written:\n{text}"
        );

        for round in 0..5 {
            let text = current.serialize();
            current = AppSettings::parse(&text);
            assert_eq!(current, settings, "round trip {}", round + 1);
            assert!(!text.contains(r"\\"), "round {} doubled a backslash", round + 1);
        }
    }

    #[test]
    fn quotes_and_newlines_still_escape_correctly() {
        let mut settings = AppSettings::default();
        settings.user_agent = "rdm \"quoted\" agent".to_string();
        let parsed = AppSettings::parse(&settings.serialize());
        assert_eq!(parsed.user_agent, settings.user_agent);

        let parsed = AppSettings::parse("download_dir = D:\\iso\\files\nclose_to_tray = true\n");
        assert_eq!(parsed.download_dir, r"D:\iso\files");
        assert!(parsed.close_to_tray);
    }

    #[test]
    fn the_new_switches_default_to_on_and_parse() {
        let defaults = AppSettings::default();
        assert!(defaults.clipboard_prefill);
        assert!(defaults.close_to_tray);
        assert!(!defaults.drop_target_enabled, "opt-in: it adds a window");

        let parsed = AppSettings::parse(
            "clipboard_prefill = false\nclose_to_tray = false\ndrop_target_enabled = true\n",
        );
        assert!(!parsed.clipboard_prefill);
        assert!(!parsed.close_to_tray);
        assert!(parsed.drop_target_enabled);
        // Bare boolean forms from hand editing are accepted too.
        assert!(AppSettings::parse("close_to_tray = yes\n").close_to_tray);
        assert!(AppSettings::parse("close_to_tray = 1\n").close_to_tray);
    }

    #[test]
    fn a_command_line_data_dir_is_not_written_back_or_overridden() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(SETTINGS_FILE),
            "data_dir = \"C:\\\\elsewhere\"\nconnections = 4\n",
        )
        .unwrap();
        let mut store = SettingsStore::new(dir.path(), true);
        assert_eq!(store.settings().data_dir, dir.path().display().to_string());
        assert_eq!(store.settings().connections, 4, "other keys still load");
        store.settings_mut().connections = 6;
        store.save().unwrap();
        let written = std::fs::read_to_string(dir.path().join(SETTINGS_FILE)).unwrap();
        assert!(
            written.contains(&format!("data_dir = \"{}\"", dir.path().display())),
            "the flag value is what the file must keep:\n{written}"
        );
    }

    #[test]
    fn form_defaults_come_from_the_settings() {
        let mut settings = AppSettings::default();
        settings.download_dir = "/downloads".to_string();
        settings.connections = 4;
        let request = settings.to_request();
        assert_eq!(request.output, "/downloads");
        assert_eq!(request.connections, 4);
        assert!(!request.resume);
        assert!(request.url.is_empty());
    }
}
