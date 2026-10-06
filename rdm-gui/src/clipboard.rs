//! Clipboard access behind a tiny, testable façade.
//!
//! Used by
//!
//! * *New download* — the URL field is pre-filled from the clipboard when it
//!   looks like a link (feature: “paste a link, press New”);
//! * the tray / drop-target *New download* entries, which take the link with
//!   them;
//! * the details modal's *Copy JSON* / *Copy CLI command* buttons.
//!
//! Every entry point degrades gracefully: a clipboard that cannot be read
//! (headless session, another app holding it) yields `None` instead of an
//! error dialog, because the field stays editable either way.

/// The clipboard text, trimmed, or `None` when it is empty/unreadable.
pub fn text() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let text = clipboard.get_text().ok()?;
    let text = text.trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Does this clipboard text look like something the download form should start
/// with? Keeps “New download” from pre-filling whatever the user copied last
/// (a password, a paragraph of text, a file path without a scheme).
pub fn looks_like_url(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() || text.contains(char::is_whitespace) {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return false;
    }
    // `url::Url` is the same parser the engine uses, so what passes here will
    // pass `engine_options` validation as a scheme/host check.
    url::Url::parse(text)
        .map(|parsed| parsed.host_str().map(|h| !h.is_empty()).unwrap_or(false))
        .unwrap_or(false)
}

/// The clipboard text if it looks like a URL.
pub fn url() -> Option<String> {
    text().filter(|text| looks_like_url(text))
}

/// Copy text; returns a human sentence for the status bar.
pub fn copy(text: &str) -> String {
    match arboard::Clipboard::new().and_then(|mut c| c.set_text(text.to_string())) {
        Ok(()) => format!("Copied {} character(s) to the clipboard.", text.chars().count()),
        Err(err) => format!("cannot copy to the clipboard: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_downloadable_links() {
        assert!(looks_like_url("https://example.com/file.zip"));
        assert!(looks_like_url("http://example.com"));
        assert!(looks_like_url("  https://example.com/a/b?c=1#d  "));
        assert!(looks_like_url("HTTPS://EXAMPLE.COM/FILE"));
    }

    #[test]
    fn rejects_anything_that_is_not_a_link() {
        assert!(!looks_like_url(""));
        assert!(!looks_like_url("   "));
        assert!(!looks_like_url("example.com/file.zip")); // no scheme
        assert!(!looks_like_url("ftp://example.com/file.zip"));
        assert!(!looks_like_url("https://")); // no host
        assert!(!looks_like_url("https://example.com and some words"));
        assert!(!looks_like_url("C:\\downloads\\file.zip"));
        assert!(!looks_like_url(&"x".repeat(5000)));
    }
}
