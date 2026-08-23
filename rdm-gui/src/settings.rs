//! Settings loader with hot reload observer.

use std::path::PathBuf;
use std::time::SystemTime;

const DEFAULT_SETTINGS_PATH: &str = ".rdm/settings.toml";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppSettings {
    pub max_connections: u32,
    pub download_dir: String,
    pub chunk_size_mb: u32,
    pub user_agent: Option<String>,
    pub retry_limit: u32,
}

pub struct SettingsObserver {
    path: PathBuf,
    last_modified: Option<SystemTime>,
    settings: AppSettings,
}

impl SettingsObserver {
    pub fn new(path: Option<PathBuf>) -> Self {
        let path = path.unwrap_or_else(|| PathBuf::from(DEFAULT_SETTINGS_PATH));
        let mut o = Self {
            path,
            last_modified: None,
            settings: AppSettings::default(),
        };
        o.load();
        o
    }

    pub fn load(&mut self) {
        if let Ok(meta) = std::fs::metadata(&self.path) {
            if let Ok(modified) = meta.modified() {
                self.last_modified = Some(modified);
            }
        }
        if let Ok(content) = std::fs::read_to_string(&self.path) {
            self.settings = Self::parse(&content);
        }
    }

    pub fn check_reload(&mut self) -> bool {
        let modified = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok();
        if modified.is_some() && modified != self.last_modified {
            self.load();
            true
        } else {
            false
        }
    }

    fn parse(content: &str) -> AppSettings {
        let mut s = AppSettings::default();
        for line in content.lines() {
            let line = line.split('#').next().unwrap_or("");
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                let v = v.trim().trim_matches('"');
                match k {
                    "max_connections" => s.max_connections = v.parse().unwrap_or(4),
                    "download_dir" => s.download_dir = v.to_string(),
                    "chunk_size_mb" => s.chunk_size_mb = v.parse().unwrap_or(10),
                    "user_agent" => s.user_agent = Some(v.to_string()),
                    "retry_limit" => s.retry_limit = v.parse().unwrap_or(3),
                    _ => {}
                }
            }
        }
        s
    }

    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut AppSettings {
        &mut self.settings
    }
}
