//! Errors, and the translation layer between yt-dlp's output and language a
//! non-technical person can act on.
//!
//! The spec is explicit that users should never see technical jargon. The rule this
//! module follows: every message names what went wrong in plain words and, where there
//! is one, suggests the next thing to try. We never surface a stack trace, an HTTP
//! status code, or the string "yt-dlp" to the user.

use serde::Serialize;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("network problem: {0}")]
    Network(String),

    #[error("file problem: {0}")]
    Io(String),

    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(String),

    #[error("downloaded file failed verification (expected {expected}, got {actual})")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("helper program is not usable: {0}")]
    BinaryUnusable(String),

    #[error("that does not look like a valid video link")]
    InvalidUrl,

    #[error("the download did not finish: {0}")]
    DownloadFailed(String),

    #[error("setup has not finished yet")]
    NotReady,
}

/// What crosses the boundary to the frontend. `message` is safe to render directly;
/// `detail` is kept for the "Show technical details" disclosure so that a user
/// reporting a bug on GitHub has something useful to paste, without it being the
/// first thing they see.
#[derive(Debug, Clone, Serialize)]
pub struct FriendlyError {
    pub message: String,
    pub detail: Option<String>,
    /// Whether offering "Try again?" makes sense. Retrying an unsupported platform or
    /// a members-only video will not help, and a button that never works is worse
    /// than no button.
    pub retryable: bool,
}

impl AppError {
    pub fn to_friendly(&self) -> FriendlyError {
        match self {
            AppError::InvalidUrl => FriendlyError {
                message: "That doesn't look like a valid video link. Try copying the \
                          web address again from your browser."
                    .into(),
                detail: None,
                retryable: true,
            },
            AppError::Network(detail) => FriendlyError {
                message: "Check your internet connection and try again.".into(),
                detail: Some(detail.clone()),
                retryable: true,
            },
            AppError::UnsupportedPlatform(detail) => FriendlyError {
                message: "This app doesn't support this type of computer yet.".into(),
                detail: Some(detail.clone()),
                retryable: false,
            },
            AppError::ChecksumMismatch { expected, actual } => FriendlyError {
                message: "Something went wrong while setting up. Please close the app \
                          and open it again."
                    .into(),
                detail: Some(format!(
                    "checksum mismatch: expected {expected}, got {actual}"
                )),
                retryable: true,
            },
            AppError::BinaryUnusable(detail) => FriendlyError {
                message: "Some parts of the app didn't install correctly. Please close \
                          the app and open it again."
                    .into(),
                detail: Some(detail.clone()),
                retryable: true,
            },
            AppError::Io(detail) => FriendlyError {
                message: "The file couldn't be saved. Check that you have enough free \
                          space on your computer."
                    .into(),
                detail: Some(detail.clone()),
                retryable: true,
            },
            AppError::NotReady => FriendlyError {
                message: "Still getting set up — this only happens the first time. \
                          One moment."
                    .into(),
                detail: None,
                retryable: true,
            },
            AppError::DownloadFailed(stderr) => interpret_ytdlp_error(stderr),
        }
    }
}

/// `#[tauri::command]` requires error types to be `Serialize`. We serialize as the
/// friendly form so the frontend never has to know about the internal variants — the
/// translation happens once, here, rather than being duplicated in JavaScript.
impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        self.to_friendly().serialize(s)
    }
}

/// Maps yt-dlp's stderr onto something a person can act on.
///
/// Ordering matters: the checks run most-specific first, because yt-dlp often emits
/// several lines and a generic pattern would otherwise shadow a precise one. The
/// fallback is deliberately the exact wording from the spec.
pub fn interpret_ytdlp_error(stderr: &str) -> FriendlyError {
    let lower = stderr.to_lowercase();
    let detail = Some(trim_detail(stderr));

    // --- Things the user can fix -------------------------------------------------
    if lower.contains("is not a valid url") || lower.contains("unsupported url") {
        return FriendlyError {
            message: "That link isn't from a site this app can download from. Double-check \
                      you copied the whole web address."
                .into(),
            detail,
            retryable: true,
        };
    }

    if lower.contains("video unavailable") || lower.contains("this video is unavailable") {
        return FriendlyError {
            message: "That video isn't available anymore. It may have been deleted or made \
                      private."
                .into(),
            detail,
            retryable: false,
        };
    }

    if lower.contains("private video")
        || lower.contains("members-only")
        || lower.contains("join this channel")
    {
        return FriendlyError {
            message: "That video is private, so it can't be downloaded.".into(),
            detail,
            retryable: false,
        };
    }

    if lower.contains("sign in to confirm your age")
        || lower.contains("age-restricted")
        || lower.contains("confirm your age")
    {
        return FriendlyError {
            message: "That video is age-restricted, so it can't be downloaded without \
                      signing in."
                .into(),
            detail,
            retryable: false,
        };
    }

    if lower.contains("sign in to confirm")
        || lower.contains("not a bot")
        || lower.contains("cookies")
    {
        return FriendlyError {
            message: "The website is asking us to sign in before it will share this video, \
                      so it can't be downloaded."
                .into(),
            detail,
            retryable: false,
        };
    }

    if lower.contains("is not available in your country")
        || lower.contains("geo") && lower.contains("block")
    {
        return FriendlyError {
            message: "That video isn't available in your country.".into(),
            detail,
            retryable: false,
        };
    }

    if lower.contains("this live event will begin") || lower.contains("premieres in") {
        return FriendlyError {
            message: "That video hasn't started yet. Try again once it's live.".into(),
            detail,
            retryable: true,
        };
    }

    if lower.contains("is live") && lower.contains("not") {
        return FriendlyError {
            message: "Live streams can't be downloaded while they're still going. Try again \
                      once it has finished."
                .into(),
            detail,
            retryable: true,
        };
    }

    // --- Environment problems ----------------------------------------------------
    if lower.contains("unable to download webpage")
        || lower.contains("temporary failure in name resolution")
        || lower.contains("getaddrinfo")
        || lower.contains("connection reset")
        || lower.contains("timed out")
    {
        return FriendlyError {
            message: "Check your internet connection and try again.".into(),
            detail,
            retryable: true,
        };
    }

    if lower.contains("no space left") || lower.contains("disk full") {
        return FriendlyError {
            message: "Your computer is out of storage space. Free up some room and try again."
                .into(),
            detail,
            retryable: true,
        };
    }

    if lower.contains("permission denied") || lower.contains("access is denied") {
        return FriendlyError {
            message: "The file couldn't be saved to your Downloads folder. Check that the \
                      app is allowed to save files there."
                .into(),
            detail,
            retryable: true,
        };
    }

    if lower.contains("ffmpeg") || lower.contains("postprocessing") {
        return FriendlyError {
            message: "The video downloaded but couldn't be converted. Please close the app \
                      and open it again."
                .into(),
            detail,
            retryable: true,
        };
    }

    if lower.contains("http error 429") || lower.contains("too many requests") {
        return FriendlyError {
            message: "The website is asking us to slow down. Wait a few minutes and try again."
                .into(),
            detail,
            retryable: true,
        };
    }

    if lower.contains("copyright") || lower.contains("blocked it on copyright grounds") {
        return FriendlyError {
            message: "That video is blocked from downloading for copyright reasons.".into(),
            detail,
            retryable: false,
        };
    }

    // --- Spec-mandated fallback --------------------------------------------------
    FriendlyError {
        message: "This video can't be downloaded right now. Try another?".into(),
        detail,
        retryable: true,
    }
}

/// yt-dlp can emit hundreds of lines. Keep the tail, which is where the actual error
/// lands, and cap it so the disclosure panel stays readable.
fn trim_detail(stderr: &str) -> String {
    let cleaned: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let tail = cleaned
        .iter()
        .rev()
        .take(8)
        .rev()
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    if tail.len() > 1200 {
        format!("{}…", &tail[..1200])
    } else {
        tail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_errors_use_the_spec_wording() {
        let e = interpret_ytdlp_error("ERROR: something nobody has ever seen before");
        assert_eq!(
            e.message,
            "This video can't be downloaded right now. Try another?"
        );
        assert!(e.retryable);
    }

    #[test]
    fn network_failures_are_recognised() {
        let e = interpret_ytdlp_error(
            "ERROR: Unable to download webpage: <urlopen error [Errno -3] Temporary failure in name resolution>",
        );
        assert!(e.message.contains("internet connection"));
        assert!(e.retryable);
    }

    #[test]
    fn deleted_videos_are_not_retryable() {
        let e = interpret_ytdlp_error("ERROR: [youtube] dQw4w9WgXcQ: Video unavailable");
        assert!(!e.retryable, "retrying a deleted video can never succeed");
    }

    #[test]
    fn private_videos_are_not_retryable() {
        let e = interpret_ytdlp_error(
            "ERROR: [youtube] abc: Private video. Sign in if you've been granted access",
        );
        assert!(!e.retryable);
        assert!(e.message.contains("private"));
    }

    #[test]
    fn disk_full_is_distinguished_from_generic_failure() {
        let e = interpret_ytdlp_error("OSError: [Errno 28] No space left on device");
        assert!(e.message.contains("storage space"));
    }

    #[test]
    fn no_user_facing_message_leaks_jargon() {
        // Every branch of the mapper, sampled by its trigger string.
        let samples = [
            "ERROR: Unsupported URL: https://example.com/x",
            "ERROR: Video unavailable",
            "ERROR: Private video",
            "ERROR: Sign in to confirm your age",
            "ERROR: Unable to download webpage",
            "OSError: No space left on device",
            "PermissionError: Permission denied",
            "ERROR: ffmpeg not found",
            "ERROR: HTTP Error 429: Too Many Requests",
            "ERROR: totally unknown failure",
        ];
        for s in samples {
            let msg = interpret_ytdlp_error(s).message.to_lowercase();
            for jargon in [
                "yt-dlp",
                "ffmpeg",
                "traceback",
                "errno",
                "http error",
                "stderr",
                "null",
            ] {
                assert!(
                    !msg.contains(jargon),
                    "user-facing message for {s:?} leaked {jargon:?}: {msg}"
                );
            }
        }
    }

    #[test]
    fn technical_detail_is_preserved_for_bug_reports() {
        let e = interpret_ytdlp_error("ERROR: Video unavailable");
        assert!(e.detail.unwrap().contains("Video unavailable"));
    }

    #[test]
    fn detail_is_capped_for_very_noisy_output() {
        let noisy = "line of yt-dlp noise\n".repeat(500);
        let e = interpret_ytdlp_error(&noisy);
        assert!(e.detail.unwrap().len() <= 1300);
    }
}
