//! Small formatting helpers (no chrono dependency).

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// `1970-01-01 00:00:00 UTC` style rendering of an epoch-millisecond stamp.
pub fn format_timestamp(ms: i64) -> String {
    if ms <= 0 {
        return "—".to_string();
    }
    let secs = ms / 1000;
    let days = secs.div_euclid(86_400);
    let time_of_day = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let (h, m, s) = (
        time_of_day / 3600,
        (time_of_day % 3600) / 60,
        time_of_day % 60,
    );
    format!("{year:04}-{month:02}-{day:02} {h:02}:{m:02}:{s:02} UTC")
}

/// `3m ago`, `just now`, `2h 5m ago`.
pub fn format_relative(ms: i64) -> String {
    if ms <= 0 {
        return "—".to_string();
    }
    let now = now_ms();
    let delta = (now - ms).max(0) / 1000;
    if delta < 5 {
        return "just now".to_string();
    }
    if delta < 60 {
        return format!("{delta}s ago");
    }
    if delta < 3600 {
        return format!("{}m ago", delta / 60);
    }
    if delta < 86_400 {
        return format!("{}h {}m ago", delta / 3600, (delta % 3600) / 60);
    }
    format!("{}d ago", delta / 86_400)
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// `1h 02m 03s` / `45s`
pub fn format_duration(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "—".to_string();
    }
    let total = secs.round() as i64;
    if total < 60 {
        return format!("{total}s");
    }
    if total < 3600 {
        return format!("{}m {:02}s", total / 60, total % 60);
    }
    format!(
        "{}h {:02}m {:02}s",
        total / 3600,
        (total % 3600) / 60,
        total % 60
    )
}

/// Days since the Unix epoch → civil (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Open a file manager / file at `path` using the platform's default handler.
pub fn open_in_file_manager(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("explorer");
        c.arg(path);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(path);
        c
    };
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path);
        c
    };
    cmd.spawn().map(|_| ())
}

/// Interpret a text-field value as an existing directory, if it is one.
pub fn existing_dir(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    if path.is_dir() {
        Some(path)
    } else {
        None
    }
}

/// Open the OS file explorer's folder picker and return the chosen directory.
///
/// Blocks while the dialog is open (the native Explorer / zenity dialog runs
/// modally). Returns `None` when the user cancels.
pub fn pick_folder(start: Option<&std::path::Path>, title: &str) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(dir) = start {
        dialog = dialog.set_directory(dir);
    }
    dialog.pick_folder()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_epoch_milliseconds() {
        assert_eq!(format_timestamp(0), "—");
        assert_eq!(format_timestamp(1_000), "1970-01-01 00:00:01 UTC");
        assert_eq!(format_timestamp(1_700_000_000_000), "2023-11-14 22:13:20 UTC");
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(9.4), "9s");
        assert_eq!(format_duration(75.0), "1m 15s");
        assert_eq!(format_duration(3725.0), "1h 02m 05s");
        assert_eq!(format_duration(f64::NAN), "—");
    }

    #[test]
    fn formats_relative_times() {
        let now = now_ms();
        assert_eq!(format_relative(0), "—");
        assert_eq!(format_relative(now), "just now");
        assert_eq!(format_relative(now - 120_000), "2m ago");
    }
}
