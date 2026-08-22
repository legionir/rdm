//! Human-readable byte formatting and parsing.

use std::fmt;

/// Format a byte count using binary units with one decimal (e.g. `1.5 MiB`).
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Format a byte rate with `/s` suffix.
pub fn human_rate(bytes_per_sec: f64) -> String {
    format!("{}/s", human_bytes(bytes_per_sec.max(0.0) as u64))
}

/// Parse a size/rate specification such as `500k`, `5MB/s`, `1GiB`, `100M`.
///
/// Accepted suffixes (case-insensitive): `b`, `k`/`kb`, `m`/`mb`, `g`/`gb`,
/// `t`/`tb` and their binary counterparts `ki`, `mib`, `gib`, `tib`.
/// A trailing `/s` is accepted and ignored.
pub fn parse_bytes(spec: &str) -> Result<u64, String> {
    let raw = spec.trim();
    if raw.is_empty() {
        return Err("empty size specification".to_string());
    }
    let s = raw
        .strip_suffix("/s")
        .or_else(|| raw.strip_suffix("/S"))
        .map(|x| x.trim())
        .unwrap_or(raw);

    let split = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());
    let (num, suffix) = s.split_at(split);
    if num.is_empty() {
        return Err(format!("invalid size specification: {spec:?}"));
    }
    let value: f64 = num
        .parse()
        .map_err(|_| format!("invalid number in size specification: {spec:?}"))?;
    if !value.is_finite() || value < 0.0 {
        return Err(format!("invalid size specification: {spec:?}"));
    }

    let suffix = suffix.trim().to_ascii_lowercase();
    let multiplier: f64 = match suffix.as_str() {
        "" | "b" => 1.0,
        "k" | "kb" => 1_000.0,
        "m" | "mb" => 1_000_000.0,
        "g" | "gb" => 1_000_000_000.0,
        "t" | "tb" => 1_000_000_000_000.0,
        "ki" | "kib" => 1024.0,
        "mi" | "mib" => 1024.0 * 1024.0,
        "gi" | "gib" => 1024.0 * 1024.0 * 1024.0,
        "ti" | "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return Err(format!("unknown size suffix {suffix:?} in {spec:?}")),
    };

    let bytes = (value * multiplier).round();
    if bytes > u64::MAX as f64 {
        return Err(format!("size specification too large: {spec:?}"));
    }
    Ok(bytes as u64)
}

/// Parse a speed specification such as `5MB/s` or `2GiB/s`.
pub fn parse_speed(spec: &str) -> Result<u64, String> {
    let bytes = parse_bytes(spec)?;
    if bytes == 0 {
        return Err("speed must be greater than zero".to_string());
    }
    Ok(bytes)
}

/// ETA formatting used by the progress UI.
pub struct Eta(pub Option<u64>);

impl fmt::Display for Eta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            None => write!(f, "--:--"),
            Some(secs) => {
                let secs = secs.max(0);
                if secs < 60 {
                    write!(f, "{secs}s")
                } else if secs < 3600 {
                    write!(f, "{}m{:02}s", secs / 60, secs % 60)
                } else {
                    write!(f, "{}h{:02}m", secs / 3600, (secs % 3600) / 60)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_bytes("0").unwrap(), 0);
        assert_eq!(parse_bytes("500").unwrap(), 500);
        assert_eq!(parse_bytes("1KB").unwrap(), 1_000);
        assert_eq!(parse_bytes("1KiB").unwrap(), 1024);
        assert_eq!(parse_bytes("5MB/s").unwrap(), 5_000_000);
        assert_eq!(parse_bytes("10gib").unwrap(), 10 * 1024 * 1024 * 1024);
        assert_eq!(parse_bytes("2GiB/s").unwrap(), 2 * 1024 * 1024 * 1024);
        assert!(parse_bytes("").is_err());
        assert!(parse_bytes("abc").is_err());
        assert!(parse_bytes("12XB").is_err());
        assert!(parse_bytes("1.5MiB").unwrap() == 1_572_864);
    }

    #[test]
    fn formats_bytes() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1023), "1023 B");
        assert_eq!(human_bytes(1024), "1.0 KiB");
        assert_eq!(human_bytes(1024 * 1024 * 10), "10.0 MiB");
        assert_eq!(human_bytes(1536 * 1024 * 1024), "1.5 GiB");
    }

    #[test]
    fn eta_format() {
        assert_eq!(format!("{}", Eta(None)), "--:--");
        assert_eq!(format!("{}", Eta(Some(45))), "45s");
        assert_eq!(format!("{}", Eta(Some(125))), "2m05s");
        assert_eq!(format!("{}", Eta(Some(3661))), "1h01m");
    }
}
