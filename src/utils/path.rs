//! Safe file naming and output-path resolution.
//!
//! Security invariants:
//! * Filenames derived from server-supplied `Content-Disposition` or the URL
//!   never contain path separators, are never `.`/`..`, and never begin with
//!   `-` (to avoid `rdm` being tricked into treating the file as an option).
//! * The final output path is canonicalized when it already exists and
//!   verbatim-resolved against its parent directory; traversal outside the
//!   chosen directory is rejected.

use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};

/// Replace every character that is unsafe on common filesystems with `_`.
pub fn sanitize_filename(name: &str) -> String {
    // Never let path separators survive: derive from the final segment.
    let base = name
        .rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(name);
    let mut out = String::with_capacity(base.len());
    for ch in base.chars() {
        let ok = ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '+' | ' ' | '(' | ')');
        if ok && ch != '\0' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim().trim_matches('.').trim();
    let trimmed = trimmed.trim_start_matches('-');
    if trimmed.is_empty() {
        "download".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Derive a safe filename from a URL path segment.
pub fn filename_from_url(url: &url::Url) -> String {
    let last = url
        .path_segments()
        .and_then(|mut s| s.next_back())
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let name = sanitize_filename(last);
    if name == "download" && url.path().len() <= 1 {
        // Fall back to host when there is no usable path segment.
        return sanitize_filename(url.host_str().unwrap_or("file"));
    }
    name
}

/// Absolute path of `child` if it stays inside `root`; otherwise error.
pub fn ensure_within(root: &Path, child: &Path) -> Result<PathBuf> {
    let root = root
        .canonicalize()
        .with_context(|| format!("cannot canonicalize {}", root.display()))?;
    let candidate = if child.is_absolute() {
        child.to_path_buf()
    } else {
        root.join(child)
    };
    // Resolve `.`/`..` lexically first, then canonicalize the parent if the
    // target does not exist yet (a download may not have been created).
    let normalized = normalize_path(&candidate);
    let anchor = normalized.parent().unwrap_or(&root);
    let canonical_parent = anchor
        .canonicalize()
        .with_context(|| format!("cannot canonicalize {}", anchor.display()))?;
    if !canonical_parent.starts_with(&root) {
        bail!(
            "output path {} escapes the download directory {}",
            normalized.display(),
            root.display()
        );
    }
    Ok(normalized)
}

/// Lexically normalize a path (no filesystem access).
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Join the output directory with a safe filename, validating the result.
pub fn resolve_output_path(
    output: Option<&Path>,
    url: &url::Url,
    disposition: Option<&str>,
    default_dir: &Path,
) -> Result<PathBuf> {
    match output {
        None => Ok(default_dir.join(filename_from_url(url))),
        Some(p) if p.extension().is_some() || p.to_str().map(|s| s.ends_with('/')).unwrap_or(false) =>
        {
            // Treated as a file path directly.
            let canonical = if p.exists() {
                p.canonicalize().context("invalid output path")?
            } else {
                p.to_path_buf()
            };
            Ok(canonical)
        }
        Some(dir) => {
            let hint = disposition
                .and_then(parse_disposition_filename)
                .or_else(|| Some(filename_from_url(url)))
                .unwrap_or_else(|| "download".to_string());
            let name = sanitize_filename(&hint);
            Ok(dir.join(name))
        }
    }
}

/// Extract `filename=` from a `Content-Disposition` header value.
pub fn parse_disposition_filename(value: &str) -> Option<String> {
    for part in value.split(';') {
        let part = part.trim();
        if let Some(stripped) = part.strip_prefix("filename*=") {
            // RFC 5987: filename*=UTF-8''encoded%20name
            let encoded = stripped.trim().trim_matches('"');
            let rest = encoded.rsplit("''").next().unwrap_or(encoded);
            if let Ok(decoded) = percent_decode(rest) {
                return Some(decoded);
            }
        } else if let Some(stripped) = part.strip_prefix("filename=") {
            let name = stripped.trim().trim_matches('"');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn percent_decode(s: &str) -> Result<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() + 1 && i + 2 < bytes.len() + 1 {
            if let Ok(hex) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(hex);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).map_err(|e| anyhow::anyhow!("bad percent-encoding: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize_filename("file.zip"), "file.zip");
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename(".."), "download");
        assert_eq!(sanitize_filename(""), "download");
        assert_eq!(sanitize_filename("-rf"), "rf");
        assert_eq!(sanitize_filename("a b+c%20d"), "a b+c_20d");
    }

    #[test]
    fn disposition_parsing() {
        assert_eq!(
            parse_disposition_filename("attachment; filename=\"foo.bin\"").as_deref(),
            Some("foo.bin")
        );
        assert_eq!(
            parse_disposition_filename("attachment; filename*=UTF-8''caf%C3%A9.txt").as_deref(),
            Some("café.txt")
        );
        assert!(parse_disposition_filename("inline").is_none());
    }

    #[test]
    fn url_filename() {
        let u = url::Url::parse("https://example.com/path/to/archive.tar.gz?x=1").unwrap();
        assert_eq!(filename_from_url(&u), "archive.tar.gz");
        let bare = url::Url::parse("https://example.com").unwrap();
        assert_eq!(filename_from_url(&bare), "example.com");
    }

    #[test]
    fn normalize() {
        assert_eq!(
            normalize_path(Path::new("/a/b/../c/./d")),
            PathBuf::from("/a/c/d")
        );
    }
}
