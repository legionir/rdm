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
    /// How often the download table is re-read from SQLite.
    pub refresh_ms: u64,
    /// Ask before removing a record.
    pub confirm_remove: bool,
    /// Delete the assembled file too when removing.
    pub purge_on_remove: bool,
    pub dark_mode: bool,
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
            refresh_ms: 600,
            confirm_remove: true,
            purge_on_remove: false,
            dark_mode: true,
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
        out.push_str(&format!("refresh_ms = {}\n", self.refresh_ms));
        out.push_str(&format!("confirm_remove = {}\n", self.confirm_remove));
        out.push_str(&format!("purge_on_remove = {}\n", self.purge_on_remove));
        out.push_str(&format!("dark_mode = {}\n", self.dark_mode));
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
            let value = value.trim().trim_matches('"').trim();
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
                "refresh_ms" => s.refresh_ms = value.parse::<u64>().unwrap_or(s.refresh_ms).clamp(100, 10_000),
                "confirm_remove" => s.confirm_remove = value == "true",
                "purge_on_remove" => s.purge_on_remove = value == "true",
                "dark_mode" => s.dark_mode = value == "true",
                _ => {}
            }
        }
        s
    }
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Loads the settings file and notices external edits (hot reload).
pub struct SettingsStore {
    path: PathBuf,
    last_modified: Option<SystemTime>,
    settings: AppSettings,
}

impl SettingsStore {
    pub fn new(data_dir: &Path) -> Self {
        let mut store = SettingsStore {
            path: data_dir.join(SETTINGS_FILE),
            last_modified: None,
            settings: AppSettings::default(),
        };
        store.settings.data_dir = data_dir.display().to_string();
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
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
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

    /// Re-target the store after the data dir changed.
    pub fn relocate(&mut self, data_dir: &Path) {
        self.path = data_dir.join(SETTINGS_FILE);
        self.last_modified = None;
        self.load();
    }
}

fn modified_at(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}
