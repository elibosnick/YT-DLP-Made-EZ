//! Parsing yt-dlp's output stream.
//!
//! yt-dlp writes a mix of progress updates, informational chatter, and our own
//! templated lines to stdout. This module turns that stream into structured events.

use std::path::PathBuf;

use serde::Serialize;

/// Markers we prepend to yt-dlp's templated output so our lines are unambiguously
/// distinguishable from anything else it prints. Chosen so they cannot occur naturally
/// in a video title or file path.
pub const PROGRESS_MARKER: &str = "@@VDPROGRESS@@";
pub const FILEPATH_MARKER: &str = "@@VDFILE@@";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DownloadProgress {
    /// 0.0–100.0.
    pub percent: f64,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub speed_bytes_per_sec: Option<f64>,
    pub eta_seconds: Option<u64>,
    /// "downloading" or "converting". Conversion (merging streams, encoding MP3)
    /// happens after all bytes are in and reports no progress of its own — without a
    /// distinct phase the bar sits at 100% looking frozen, which reads as a crash.
    pub phase: &'static str,
}

impl DownloadProgress {
    pub fn converting() -> Self {
        Self {
            percent: 100.0,
            downloaded_bytes: 0,
            total_bytes: None,
            speed_bytes_per_sec: None,
            eta_seconds: None,
            phase: "converting",
        }
    }
}

/// yt-dlp renders unknown template fields as the literal string "NA".
fn field(raw: &str) -> Option<&str> {
    match raw.trim() {
        "" | "NA" | "None" | "none" => None,
        v => Some(v),
    }
}

/// Parses one `@@VDPROGRESS@@`-prefixed line. Returns `None` for anything else.
pub fn parse_progress_line(line: &str) -> Option<DownloadProgress> {
    let payload = line.trim().strip_prefix(PROGRESS_MARKER)?;
    let parts: Vec<&str> = payload.split('|').collect();
    if parts.len() < 5 {
        return None;
    }

    // yt-dlp emits downloaded_bytes as a float on some extractors.
    let downloaded_bytes = field(parts[0])?.parse::<f64>().ok()? as u64;

    // Prefer the exact total; fall back to the estimate for sources that send no
    // Content-Length. Without the fallback those downloads show 0% throughout.
    let total_bytes: Option<u64> = field(parts[1])
        .and_then(|v| v.parse::<f64>().ok())
        .or_else(|| field(parts[2]).and_then(|v| v.parse::<f64>().ok()))
        .map(|f| f as u64);

    let speed_bytes_per_sec = field(parts[3]).and_then(|v| v.parse::<f64>().ok());
    let eta_seconds = field(parts[4])
        .and_then(|v| v.parse::<f64>().ok())
        .map(|f| f as u64);

    let percent = match total_bytes {
        Some(total) if total > 0 => {
            ((downloaded_bytes as f64 / total as f64) * 100.0).clamp(0.0, 100.0)
        }
        _ => 0.0,
    };

    Some(DownloadProgress {
        percent,
        downloaded_bytes,
        total_bytes,
        speed_bytes_per_sec,
        eta_seconds,
        phase: "downloading",
    })
}

/// Detects the handoff from downloading to ffmpeg post-processing.
pub fn is_conversion_line(line: &str) -> bool {
    let l = line.to_lowercase();
    l.contains("[merger]")
        || l.contains("[extractaudio]")
        || l.contains("[videoconvertor]")
        || l.contains("[fixupm3u8]")
        || l.contains("merging formats into")
}

/// Extracts the final file path.
///
/// Two sources, because `after_move:` does not fire when yt-dlp skips a file it has
/// already downloaded — and without the second case a re-download reports "finished
/// but the file could not be located", which is both wrong and alarming.
pub fn parse_filepath_line(line: &str) -> Option<PathBuf> {
    let t = line.trim();

    if let Some(rest) = t.strip_prefix(FILEPATH_MARKER) {
        let rest = rest.trim();
        if !rest.is_empty() {
            return Some(PathBuf::from(rest));
        }
    }

    if let Some(idx) = t.find(" has already been downloaded") {
        let prefix = t[..idx].trim().trim_start_matches("[download]").trim();
        if !prefix.is_empty() {
            return Some(PathBuf::from(prefix));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(payload: &str) -> String {
        format!("{PROGRESS_MARKER}{payload}")
    }

    #[test]
    fn parses_a_normal_progress_line() {
        let p = parse_progress_line(&line("524288|1048576|NA|131072.5|4")).unwrap();
        assert_eq!(p.downloaded_bytes, 524_288);
        assert_eq!(p.total_bytes, Some(1_048_576));
        assert!((p.percent - 50.0).abs() < 0.001);
        assert_eq!(p.speed_bytes_per_sec, Some(131_072.5));
        assert_eq!(p.eta_seconds, Some(4));
        assert_eq!(p.phase, "downloading");
    }

    #[test]
    fn falls_back_to_the_size_estimate() {
        // Many sites omit Content-Length. Without this fallback the bar would sit at
        // 0% for the whole download and look broken.
        let p = parse_progress_line(&line("250|NA|1000.0|NA|NA")).unwrap();
        assert_eq!(p.total_bytes, Some(1000));
        assert!((p.percent - 25.0).abs() < 0.001);
    }

    #[test]
    fn handles_a_completely_unknown_size() {
        let p = parse_progress_line(&line("250|NA|NA|NA|NA")).unwrap();
        assert_eq!(p.percent, 0.0);
        assert_eq!(p.total_bytes, None);
        assert_eq!(p.speed_bytes_per_sec, None);
        assert_eq!(p.eta_seconds, None);
    }

    #[test]
    fn accepts_float_byte_counts() {
        // Some extractors report floats where the template implies an integer.
        let p = parse_progress_line(&line("524288.0|1048576.0|NA|NA|NA")).unwrap();
        assert_eq!(p.downloaded_bytes, 524_288);
        assert_eq!(p.total_bytes, Some(1_048_576));
    }

    #[test]
    fn percent_never_exceeds_one_hundred() {
        // yt-dlp can report downloaded > total on some fragmented streams; a progress
        // bar rendering 140% looks like a bug to the user.
        assert_eq!(
            parse_progress_line(&line("2000|1000|NA|NA|NA"))
                .unwrap()
                .percent,
            100.0
        );
    }

    #[test]
    fn ignores_unrelated_output() {
        for l in [
            "[youtube] Extracting URL: https://youtube.com/watch?v=x",
            "[download] Destination: video.mp4",
            "[download]  50.0% of 10.00MiB at 1.00MiB/s ETA 00:05",
            "",
            "   ",
            "@@VDPROGRESS@@malformed",
            "@@VDPROGRESS@@1|2|3",
        ] {
            assert!(parse_progress_line(l).is_none(), "should ignore {l:?}");
        }
    }

    #[test]
    fn tolerates_trailing_whitespace() {
        assert!(parse_progress_line(&format!("{}\r", line("1|2|NA|NA|NA"))).is_some());
    }

    #[test]
    fn captures_the_final_path() {
        let p = parse_filepath_line("@@VDFILE@@/home/gran/Downloads/Cat Video.mp4").unwrap();
        assert_eq!(p, PathBuf::from("/home/gran/Downloads/Cat Video.mp4"));
    }

    #[test]
    fn captures_the_path_of_an_already_downloaded_file() {
        // after_move never fires here, so without this branch re-downloading a video
        // reports failure despite the file being right there.
        let p = parse_filepath_line(
            "[download] /home/gran/Downloads/Cat Video.mp4 has already been downloaded",
        )
        .unwrap();
        assert_eq!(p, PathBuf::from("/home/gran/Downloads/Cat Video.mp4"));
    }

    #[test]
    fn handles_windows_paths_with_spaces() {
        let p =
            parse_filepath_line(r"@@VDFILE@@C:\Users\Gran\Downloads\My Holiday Video.mp4").unwrap();
        assert!(p.to_string_lossy().ends_with("My Holiday Video.mp4"));
    }

    #[test]
    fn handles_unicode_titles() {
        // Non-ASCII titles are extremely common and a naive byte-slicing parser breaks
        // on them.
        let p = parse_filepath_line("@@VDFILE@@/home/gran/Downloads/Café — Ñoño 日本.mp4").unwrap();
        assert!(p.to_string_lossy().contains("Café — Ñoño 日本"));
    }

    #[test]
    fn ignores_lines_with_no_path() {
        for l in ["@@VDFILE@@", "@@VDFILE@@   ", "[download] 100%", ""] {
            assert!(parse_filepath_line(l).is_none(), "should ignore {l:?}");
        }
    }

    #[test]
    fn detects_the_conversion_phase() {
        assert!(is_conversion_line(
            "[Merger] Merging formats into \"video.mp4\""
        ));
        assert!(is_conversion_line("[ExtractAudio] Destination: song.mp3"));
        assert!(is_conversion_line("[VideoConvertor] Converting video"));
        assert!(!is_conversion_line("[download] 50.0% of 10.00MiB"));
        assert!(!is_conversion_line("[youtube] Extracting URL"));
    }
}
