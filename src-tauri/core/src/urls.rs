//! URL validation.

use crate::error::{AppError, Result};

/// Cheap structural check so we can show the spec's "that doesn't look like a valid
/// video link" message immediately, without the cost and latency of asking yt-dlp.
///
/// This deliberately does not try to know which of yt-dlp's 1000+ supported sites are
/// valid — that list changes constantly and a stale allowlist would reject working
/// links. Anything structurally URL-shaped is handed to yt-dlp, which is the real
/// authority on what it can download.
///
/// Security note: the http/https restriction is not cosmetic. yt-dlp happily accepts
/// `file://` and would read the local disk; this is the only thing standing between a
/// pasted string and that behaviour.
pub fn validate_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim();

    if trimmed.is_empty() || trimmed.len() > 2048 {
        return Err(AppError::InvalidUrl);
    }

    // People paste what the address bar shows them, which often omits the scheme.
    // Rejecting "youtube.com/watch?v=x" would be needlessly pedantic.
    let candidate = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.contains("://") {
        // Some other scheme (file:, ftp:, javascript:). Reject rather than rewrite —
        // silently "upgrading" file:// to https://file would be surprising.
        return Err(AppError::InvalidUrl);
    } else if trimmed.contains(':') && !trimmed.contains('/') {
        // Catches scheme-like input without a slash, e.g. "javascript:alert(1)".
        return Err(AppError::InvalidUrl);
    } else {
        format!("https://{trimmed}")
    };

    let url = url::Url::parse(&candidate).map_err(|_| AppError::InvalidUrl)?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::InvalidUrl);
    }

    let host = url.host_str().ok_or(AppError::InvalidUrl)?;

    // A host with no dot is not a real public site — this rejects both typos
    // ("youtube") and internal names ("localhost", "router").
    if !host.contains('.') || host.starts_with('.') || host.ends_with('.') {
        return Err(AppError::InvalidUrl);
    }

    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_video_links() {
        for url in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "http://youtu.be/dQw4w9WgXcQ",
            "https://www.tiktok.com/@user/video/123",
            "https://vimeo.com/12345",
            "https://www.twitch.tv/videos/12345",
            "https://www.instagram.com/reel/abc/",
            "https://x.com/user/status/123",
        ] {
            assert!(validate_url(url).is_ok(), "should accept {url}");
        }
    }

    #[test]
    fn tolerates_surrounding_whitespace() {
        // Copying from a chat message very often brings a trailing newline with it.
        assert!(validate_url("  https://youtu.be/x \n").is_ok());
    }

    #[test]
    fn adds_the_scheme_to_bare_hosts() {
        let out = validate_url("youtube.com/watch?v=abc").unwrap();
        assert!(out.starts_with("https://"), "got {out}");
    }

    #[test]
    fn rejects_nonsense() {
        for bad in [
            "",
            "   ",
            "hello world",
            "not a url",
            "youtube",
            "..",
            "http://",
        ] {
            assert!(validate_url(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn rejects_non_web_schemes() {
        // Regression guard with teeth: a file:// URL reaching yt-dlp would let a pasted
        // string read the local filesystem.
        for bad in [
            "file:///etc/passwd",
            "ftp://example.com/x",
            "javascript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
        ] {
            assert!(validate_url(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn rejects_hosts_without_a_dot() {
        for bad in [
            "http://localhost/x",
            "https://router/admin",
            "localhost:8080",
        ] {
            assert!(validate_url(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn rejects_absurdly_long_input() {
        let long = format!("https://example.com/{}", "a".repeat(3000));
        assert!(validate_url(&long).is_err());
    }

    #[test]
    fn preserves_query_parameters() {
        // Timestamps and playlist indices live in the query string; dropping them would
        // silently change what the user gets.
        let out = validate_url("https://youtu.be/x?t=42").unwrap();
        assert!(out.contains("t=42"), "got {out}");
    }
}
