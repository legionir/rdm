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

/// Derive a safe filename from a URL.
///
/// Signed download URLs (notably GitHub release assets) often carry an opaque
/// final path segment and put the real name in a content-disposition query
/// parameter.  Prefer that hint, then use the final path segment.
pub fn filename_from_url(url: &url::Url) -> String {
    if let Some(name) = filename_from_url_disposition(url) {
        return sanitize_filename(&name);
    }

    let last = url
        .path_segments()
        .and_then(|mut s| s.next_back())
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    // URL paths are percent-encoded.  Decode before sanitising so a name such
    // as `release%20notes.txt` does not turn into `release_20notes.txt`.
    let decoded = percent_decode(last).unwrap_or_else(|_| last.to_string());
    let name = sanitize_filename(&decoded);
    if name == "download" && url.path().len() <= 1 {
        // Fall back to host when there is no usable path segment.
        return sanitize_filename(url.host_str().unwrap_or("file"));
    }
    name
}

/// Extract a filename from disposition-bearing URL query parameters.
///
/// Azure/GitHub signed asset URLs use both `response-content-disposition` and
/// its abbreviated `rscd` form.  `Url::query_pairs` performs the outer URL
/// decoding; the resulting value is parsed exactly like the HTTP header.
pub fn filename_from_url_disposition(url: &url::Url) -> Option<String> {
    url.query_pairs()
        .filter(|(key, _)| {
            key.eq_ignore_ascii_case("response-content-disposition")
                || key.eq_ignore_ascii_case("rscd")
        })
        .find_map(|(_, value)| parse_disposition_filename(&value))
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
/// `disposition_filename` is the value already extracted by the network probe,
/// not a complete `Content-Disposition` header.
pub fn resolve_output_path(
    output: Option<&Path>,
    url: &url::Url,
    disposition_filename: Option<&str>,
    default_dir: &Path,
) -> Result<PathBuf> {
    match output {
        None => {
            let hint = disposition_filename
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| filename_from_url(url));
            Ok(default_dir.join(sanitize_filename(&hint)))
        }
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
            let hint = disposition_filename
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| filename_from_url(url));
            Ok(dir.join(sanitize_filename(&hint)))
        }
    }
}

/// Extract `filename=` / `filename*=` from a `Content-Disposition` value.
///
/// Parameter names are ASCII case-insensitive and may have whitespace around
/// `=`.  RFC 5987's extended filename wins over the plain fallback regardless
/// of parameter order.
pub fn parse_disposition_filename(value: &str) -> Option<String> {
    let mut plain = None;
    for part in value.split(';') {
        let Some((key, stripped)) = part.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let stripped = stripped.trim().trim_matches('"');
        if key.eq_ignore_ascii_case("filename*") {
            // RFC 5987: filename*=UTF-8''encoded%20name
            let rest = stripped.rsplit("''").next().unwrap_or(stripped);
            if let Ok(decoded) = percent_decode(rest) {
                if !decoded.is_empty() {
                    return Some(decoded);
                }
            }
        } else if key.eq_ignore_ascii_case("filename") && !stripped.is_empty() {
            plain = Some(stripped.to_string());
        }
    }
    plain
}

fn percent_decode(s: &str) -> Result<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
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
        let encoded = url::Url::parse("https://example.com/release%20notes.txt").unwrap();
        assert_eq!(filename_from_url(&encoded), "release notes.txt");
    }

    #[test]
    fn github_asset_query_filename_beats_opaque_path() {
        let url = url::Url::parse(
            "https://release-assets.githubusercontent.com/05386eb0-c1f1-4c46-a9f9-ef62d9dfa65f?rscd=attachment%3B+filename%3Dventoy-1.1.18-windows.zip&response-content-disposition=attachment%3B%20filename%3Dventoy-1.1.18-windows.zip",
        )
        .unwrap();
        assert_eq!(
            filename_from_url_disposition(&url).as_deref(),
            Some("ventoy-1.1.18-windows.zip")
        );
        assert_eq!(filename_from_url(&url), "ventoy-1.1.18-windows.zip");
        assert_eq!(
            resolve_output_path(None, &url, None, Path::new("downloads")).unwrap(),
            PathBuf::from("downloads/ventoy-1.1.18-windows.zip")
        );
    }

    #[test]
    fn probed_header_filename_has_priority_for_output_directories() {
        let url = url::Url::parse("https://example.com/opaque?id=1").unwrap();
        // The network layer has already parsed the header.  This used to be
        // parsed a second time here, making a plain filename disappear.
        let probed_filename = Some("release.zip");
        assert_eq!(
            resolve_output_path(None, &url, probed_filename, Path::new("default")).unwrap(),
            PathBuf::from("default/release.zip")
        );
        assert_eq!(
            resolve_output_path(
                Some(Path::new("chosen")),
                &url,
                probed_filename,
                Path::new("default")
            )
            .unwrap(),
            PathBuf::from("chosen/release.zip")
        );
    }

    #[test]
    fn extended_disposition_wins_over_plain_filename() {
        assert_eq!(
            parse_disposition_filename(
                "attachment; filename=plain.txt; Filename* = UTF-8''caf%C3%A9.txt"
            )
            .as_deref(),
            Some("café.txt")
        );
    }

    #[test]
    fn normalize() {
        assert_eq!(
            normalize_path(Path::new("/a/b/../c/./d")),
            PathBuf::from("/a/c/d")
        );
    }
}
