//! HTTP Range semantics: parsing, validation and chunk planning helpers.

use anyhow::{anyhow, bail, Result};
use reqwest::header::{HeaderMap, ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE};

/// Parsed `Content-Range: bytes <start>-<end>/<total|*>` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentRange {
    pub start: u64,
    pub end: u64,
    pub total: Option<u64>,
}

pub fn parse_content_range(value: &str) -> Option<ContentRange> {
    let value = value.trim();
    let rest = value.strip_prefix("bytes ")?;
    let (range, total) = rest.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start: u64 = start.trim().parse().ok()?;
    let end: u64 = end.trim().parse().ok()?;
    let total = match total.trim() {
        "*" => None,
        other => Some(other.parse().ok()?),
    };
    Some(ContentRange { start, end, total })
}

/// Parse `bytes */<total>` (the `416` response shape).
pub fn parse_unsatisfied_size(value: &str) -> Option<u64> {
    let rest = value.trim().strip_prefix("bytes */")?;
    rest.trim().parse().ok()
}

/// True when the `Accept-Ranges` header advertises byte range support.
pub fn accept_ranges_supported(headers: &HeaderMap) -> bool {
    headers
        .get_all(ACCEPT_RANGES)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .any(|v| {
            v.split(',').any(|part| part.trim().eq_ignore_ascii_case("bytes"))
        })
}

/// Parse `Content-Length` when present.
pub fn content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// Classification of a response to a ranged request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeResponse {
    /// Server honored `Range` and returned exactly the requested window.
    Partial { range: ContentRange },
    /// Server ignored the header and returned the full entity (200).
    Full { total: Option<u64> },
    /// `416 Range Not Satisfiable` — the requested offset is at/after EOF.
    NotSatisfiable { claimed_size: Option<u64> },
}

/// Validate the HTTP response against a requested inclusive range `[start, end]`.
///
/// `allow_full` controls whether a `200 OK` is accepted (single-stream mode).
pub fn classify_range_response(
    status: reqwest::StatusCode,
    headers: &HeaderMap,
    start: u64,
    end: u64,
) -> Result<RangeResponse> {
    if status == reqwest::StatusCode::PARTIAL_CONTENT {
        let value = headers
            .get(CONTENT_RANGE)
            .ok_or_else(|| anyhow!("206 response without Content-Range header"))?
            .to_str()
            .map_err(|_| anyhow!("non-UTF8 Content-Range header"))?;
        let range =
            parse_content_range(value).ok_or_else(|| anyhow!("malformed Content-Range: {value:?}"))?;
        if range.start != start || range.end > end {
            bail!(
                "server returned unexpected window bytes={}-{} (requested {}-{})",
                range.start,
                range.end,
                start,
                end
            );
        }
        return Ok(RangeResponse::Partial { range });
    }

    if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        let claimed = headers
            .get(CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_unsatisfied_size);
        return Ok(RangeResponse::NotSatisfiable { claimed_size: claimed });
    }

    if status == reqwest::StatusCode::OK {
        return Ok(RangeResponse::Full {
            total: content_length(headers),
        });
    }

    // `Range` may be silently dropped by a proxy for only some status codes;
    // treat everything else as a protocol error rather than silently mis-saving.
    bail!("unexpected HTTP status {} for ranged request", status);
}

/// A planned byte range for one chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    /// Inclusive.
    pub end: u64,
}

impl ByteRange {
    pub fn len(&self) -> u64 {
        self.end - self.start + 1
    }

    pub fn is_empty(&self) -> bool {
        self.end < self.start
    }

    pub fn header_value(&self) -> String {
        format!("bytes={}-{}", self.start, self.end)
    }
}

/// Minimum size of a dynamically split chunk region.
pub const MIN_CHUNK_BYTES: u64 = 256 * 1024;

/// Split `total` bytes into at most `pieces` roughly equal chunks.
pub fn plan_chunks(total: u64, pieces: usize, min_chunk: u64) -> Vec<ByteRange> {
    if total == 0 {
        return Vec::new();
    }
    let min_chunk = min_chunk.max(MIN_CHUNK_BYTES).max(1);
    // Never create more chunks than `total / min_chunk` (that would defeat the
    // minimum granularity), but always split small files into at least one.
    let max_pieces = (if total < min_chunk { total } else { total / min_chunk }).max(1);
    let pieces = (pieces.max(1) as u64).min(max_pieces);
    let size = total.div_ceil(pieces);
    let mut chunks = Vec::with_capacity(pieces as usize);
    let mut start = 0u64;
    while start < total {
        let end = (start + size - 1).min(total - 1);
        chunks.push(ByteRange { start, end });
        start = end + 1;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;
    use reqwest::StatusCode;

    #[test]
    fn parses_content_range() {
        assert_eq!(
            parse_content_range("bytes 0-99/1000"),
            Some(ContentRange { start: 0, end: 99, total: Some(1000) })
        );
        assert_eq!(
            parse_content_range("bytes 43000000-100000000/*"),
            Some(ContentRange { start: 43_000_000, end: 100_000_000, total: None })
        );
        assert_eq!(parse_content_range("bytes x-y/1"), None);
        assert_eq!(parse_content_range("nonsense"), None);
    }

    #[test]
    fn accept_ranges() {
        let mut h = HeaderMap::new();
        h.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        assert!(accept_ranges_supported(&h));
        h.insert(ACCEPT_RANGES, HeaderValue::from_static("none"));
        h.remove(ACCEPT_RANGES);
        h.insert(ACCEPT_RANGES, HeaderValue::from_static("none"));
    }

    #[test]
    fn classify_partial() {
        let mut h = HeaderMap::new();
        h.insert(CONTENT_RANGE, HeaderValue::from_static("bytes 10-19/100"));
        let got = classify_range_response(StatusCode::PARTIAL_CONTENT, &h, 10, 19).unwrap();
        assert!(matches!(got, RangeResponse::Partial { .. }));
        // Wrong window must be rejected.
        assert!(classify_range_response(StatusCode::PARTIAL_CONTENT, &h, 0, 19).is_err());
    }

    #[test]
    fn classify_full_and_416() {
        let mut h = HeaderMap::new();
        h.insert(CONTENT_LENGTH, HeaderValue::from_static("100"));
        assert_eq!(
            classify_range_response(StatusCode::OK, &h, 0, 9).unwrap(),
            RangeResponse::Full { total: Some(100) }
        );
        let mut h416 = HeaderMap::new();
        h416.insert(CONTENT_RANGE, HeaderValue::from_static("bytes */100"));
        assert_eq!(
            classify_range_response(StatusCode::RANGE_NOT_SATISFIABLE, &h416, 100, 199).unwrap(),
            RangeResponse::NotSatisfiable { claimed_size: Some(100) }
        );
    }

    #[test]
    fn planning() {
        let p = plan_chunks(1000, 3, 100);
        assert_eq!(p.len(), 3);
        assert_eq!(p.iter().map(|c| c.len()).sum::<u64>(), 1000);
        assert_eq!(p[0].start, 0);
        assert_eq!(p.last().unwrap().end, 999);
        assert!(plan_chunks(0, 4, 100).is_empty());
        // Single-byte file produces a single chunk even for many pieces.
        assert_eq!(plan_chunks(1, 100, 100).len(), 1);
    }

    #[test]
    fn range_header() {
        let r = ByteRange { start: 0, end: 1023 };
        assert_eq!(r.header_value(), "bytes=0-1023");
    }
}

#[cfg(test)]
mod extra_range_tests {
    use super::*;

    #[test]
    fn byte_range_header_value() {
        let r = ByteRange { start: 100, end: 299 };
        assert_eq!(r.header_value(), "bytes=100-299");
    }

    #[test]
    fn chunk_plan_empty_file() {
        assert!(plan_chunks(0, 4, 1024).is_empty());
    }
}
