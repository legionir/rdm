//! HTTP client construction and resource probing.

use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::header::{CONTENT_DISPOSITION, ETAG, LAST_MODIFIED, RANGE};
use reqwest::{Client, Method, StatusCode};

use crate::network::range::{
    accept_ranges_supported, content_length, classify_range_response, RangeResponse,
};
use crate::utils::path::{filename_from_url_disposition, parse_disposition_filename};

pub const DEFAULT_USER_AGENT: &str = concat!("rdm/", env!("CARGO_PKG_VERSION"));

/// Tunables for the HTTP layer.
#[derive(Debug, Clone)]
pub struct HttpOptions {
    pub connect_timeout: Duration,
    pub request_timeout: Option<Duration>,
    pub user_agent: String,
    /// Max idle pooled connections per host (set to connection count + slack).
    pub pool_max_idle_per_host: usize,
    pub accept: bool,
}

impl Default for HttpOptions {
    fn default() -> Self {
        HttpOptions {
            connect_timeout: Duration::from_secs(30),
            request_timeout: None,
            user_agent: DEFAULT_USER_AGENT.to_string(),
            pool_max_idle_per_host: 16,
            accept: true,
        }
    }
}

/// Build the shared `reqwest` client (rustls + webpki roots, HTTP/1.1 + HTTP/2).
pub fn build_client(opts: &HttpOptions) -> Result<Client> {
    let mut builder = Client::builder()
        .user_agent(opts.user_agent.clone())
        .connect_timeout(opts.connect_timeout)
        .pool_max_idle_per_host(opts.pool_max_idle_per_host)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(30))
        .http1_title_case_headers();
    if let Some(t) = opts.request_timeout {
        builder = builder.timeout(t);
    }
    builder
        .build()
        .context("failed to construct HTTP client (TLS backend unavailable?)")
}

/// Result of probing a URL before chunk scheduling.
#[derive(Debug, Clone)]
pub struct Probe {
    /// Final URL after redirects.
    pub effective_url: String,
    /// Total size when known.
    pub size: Option<u64>,
    /// Server advertises `Accept-Ranges: bytes` (or answered a Range probe).
    pub accept_ranges: bool,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    /// Filename hinted by `Content-Disposition`, or by its signed-URL query
    /// equivalent when the header is absent.
    pub disposition_filename: Option<String>,
}

/// Probe a resource with HEAD, then a 1-byte Range GET as fallback.
pub async fn probe(client: &Client, url: &str) -> Result<Probe> {
    // 1) HEAD: cheap, but some servers reject it or omit size.
    if let Some(p) = try_head(client, url).await? {
        if p.size.is_some() || p.accept_ranges {
            return Ok(p);
        }
    }

    // 2) GET with Range: bytes=0-0 — authoritative for size/range support.
    let resp = client
        .get(url)
        .header(RANGE, "bytes=0-0")
        .send()
        .await
        .with_context(|| format!("GET probe failed for {url}"))?;
    let effective_url = resp.url().to_string();
    let headers = resp.headers().clone();
    let status = resp.status();
    drop(resp);

    let (size, accept_ranges) = match classify_range_response(status, &headers, 0, 0) {
        Ok(RangeResponse::Partial { range }) => (range.total, true),
        Ok(RangeResponse::Full { total }) => (total, false),
        Ok(RangeResponse::NotSatisfiable { claimed_size }) => {
            // A 416 for bytes=0-0 means an existing zero-byte/redirected oddity;
            // treat as no range support.
            (claimed_size, false)
        }
        Err(_) => (content_length(&headers), accept_ranges_supported(&headers)),
    };

    Ok(Probe {
        effective_url,
        size,
        accept_ranges,
        etag: header_str(&headers, ETAG),
        last_modified: header_str(&headers, LAST_MODIFIED),
        disposition_filename: response_filename(&headers, &effective_url, url),
    })
}

async fn try_head(client: &Client, url: &str) -> Result<Option<Probe>> {
    let req = client.request(Method::HEAD, url).build()?;
    let resp = client
        .execute(req)
        .await
        .with_context(|| format!("HEAD probe failed for {url}"))?;
    let status = resp.status();
    if status.is_server_error() || status == StatusCode::METHOD_NOT_ALLOWED {
        return Ok(None);
    }
    let headers = resp.headers().clone();
    let effective_url = resp.url().to_string();
    Ok(Some(Probe {
        effective_url,
        size: content_length(&headers),
        accept_ranges: accept_ranges_supported(&headers),
        etag: header_str(&headers, ETAG),
        last_modified: header_str(&headers, LAST_MODIFIED),
        disposition_filename: response_filename(&headers, &effective_url, url),
    }))
}

/// A real header is authoritative.  Signed URLs can alternatively carry the
/// same header in their query string; retain that hint before an early HEAD
/// return, otherwise an opaque object key becomes the downloaded filename.
fn response_filename(
    headers: &reqwest::header::HeaderMap,
    effective_url: &str,
    request_url: &str,
) -> Option<String> {
    header_str(headers, CONTENT_DISPOSITION)
        .as_deref()
        .and_then(parse_disposition_filename)
        .or_else(|| {
            url::Url::parse(effective_url)
                .ok()
                .as_ref()
                .and_then(filename_from_url_disposition)
        })
        .or_else(|| {
            url::Url::parse(request_url)
                .ok()
                .as_ref()
                .and_then(filename_from_url_disposition)
        })
}

fn header_str(headers: &reqwest::header::HeaderMap, name: reqwest::header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_header_filename_wins_over_url_query() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            CONTENT_DISPOSITION,
            reqwest::header::HeaderValue::from_static(
                "attachment; filename=ventoy-1.1.18-windows.zip",
            ),
        );
        assert_eq!(
            response_filename(
                &headers,
                "https://example.com/opaque?rscd=attachment%3B+filename%3Dother.zip",
                "https://example.com/opaque"
            )
            .as_deref(),
            Some("ventoy-1.1.18-windows.zip")
        );
    }

    #[test]
    fn signed_url_query_is_used_when_the_header_is_absent() {
        let headers = reqwest::header::HeaderMap::new();
        assert_eq!(
            response_filename(
                &headers,
                "https://example.com/05386eb0-c1f1-4c46-a9f9-ef62d9dfa65f",
                "https://example.com/05386eb0-c1f1-4c46-a9f9-ef62d9dfa65f?response-content-disposition=attachment%3B%20filename%3Dventoy-1.1.18-windows.zip"
            )
            .as_deref(),
            Some("ventoy-1.1.18-windows.zip")
        );
    }
}
