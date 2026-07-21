//! Format presets and the yt-dlp command line.
//!
//! Kept pure (no `AppHandle`, no spawning) so the argument list — which is easy to
//! break and hard to notice breaking — is covered by tests.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::progress::{FILEPATH_MARKER, PROGRESS_MARKER};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    /// Spec: "Best Quality (Video + Audio)" — the default, MP4.
    BestQuality,
    /// Spec: "Audio Only (MP3)".
    AudioMp3,
    /// Spec: "Best Available (Might be huge)".
    BestAvailable,
}

impl Format {
    /// The format-specific portion of the yt-dlp arguments.
    ///
    /// The selectors use `/` fallback chains so a site that does not offer the ideal
    /// combination still yields something. For this audience a slightly lower-quality
    /// file beats an error message.
    pub fn args(self) -> Vec<String> {
        let s = |v: &str| v.to_string();
        match self {
            // mp4 video + m4a audio can be merged by remuxing rather than re-encoding,
            // so this is both fast and lossless. The fallbacks give up the container
            // preference before they give up on downloading at all.
            Format::BestQuality => vec![
                s("-f"),
                s("bv*[ext=mp4]+ba[ext=m4a]/bv*+ba/b[ext=mp4]/b"),
                s("--merge-output-format"),
                s("mp4"),
            ],
            Format::AudioMp3 => vec![
                s("-f"),
                s("ba/b"),
                s("--extract-audio"),
                s("--audio-format"),
                s("mp3"),
                // 0 = best VBR. The spec offers the user no quality choice, so we pick
                // the good one rather than inheriting yt-dlp's default of 5.
                s("--audio-quality"),
                s("0"),
            ],
            // No container constraint: mkv holds any codec combination, so this never
            // forces a re-encode just to fit the box.
            Format::BestAvailable => {
                vec![s("-f"), s("bv*+ba/b"), s("--merge-output-format"), s("mkv")]
            }
        }
    }
}

/// The full yt-dlp argument list, minus the trailing `-- <url>`.
pub fn build_ytdlp_args(format: Format, ffmpeg: &Path, out_dir: &Path) -> Vec<String> {
    let s = |v: &str| v.to_string();

    let mut args = vec![
        // The spec lists playlists as an explicit non-feature. Without this, a link
        // carrying a `&list=` parameter downloads the entire playlist — potentially
        // hundreds of files — with no way for the user to stop it.
        s("--no-playlist"),
        // `--print` implies `--simulate` and `--quiet`. These three undo that, so we
        // still actually download and still get progress on stdout. Dropping
        // --no-simulate would make the app report success having downloaded nothing.
        s("--no-simulate"),
        s("--progress"),
        // One progress line per update instead of \r-overwriting a single line, which
        // is what makes the stream parseable at all.
        s("--newline"),
        s("--progress-template"),
        format!(
            "download:{PROGRESS_MARKER}%(progress.downloaded_bytes)s|%(progress.total_bytes)s|\
             %(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s"
        ),
        s("--print"),
        format!("after_move:{FILEPATH_MARKER}%(filepath)s"),
        // Point at our own bundled ffmpeg. Without this yt-dlp silently falls back to
        // a system ffmpeg, or to none — and "none" means MP3 and merged downloads fail.
        s("--ffmpeg-location"),
        ffmpeg.to_string_lossy().to_string(),
        s("--paths"),
        format!("home:{}", out_dir.to_string_lossy()),
        // A plain, human filename — not yt-dlp's default, which appends the video id.
        // "Cat Video.mp4" is what this audience expects to find in Downloads.
        s("--output"),
        s("%(title)s.%(ext)s"),
        // Apply Windows' filename restrictions on every platform, so a title
        // containing ':' or '?' cannot produce an unopenable file or a path error.
        s("--windows-filenames"),
        // Long titles otherwise exceed the 255-byte name limit on most filesystems.
        s("--trim-filenames"),
        s("150"),
        s("--retries"),
        s("3"),
        s("--fragment-retries"),
        s("3"),
        // Without this a failed download leaves .part files littering Downloads, which
        // is both confusing and looks like the app is broken.
        s("--no-keep-fragments"),
    ];

    args.extend(format.args());
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const ALL: [Format; 3] = [Format::BestQuality, Format::AudioMp3, Format::BestAvailable];

    fn args_for(f: Format) -> Vec<String> {
        build_ytdlp_args(f, Path::new("/tmp/ffmpeg"), Path::new("/tmp/dl"))
    }

    fn value_after(args: &[String], flag: &str) -> String {
        let i = args
            .iter()
            .position(|a| a == flag)
            .unwrap_or_else(|| panic!("{flag} missing from {args:?}"));
        args[i + 1].clone()
    }

    #[test]
    fn playlists_are_always_disabled() {
        // Regression guard: losing this flag turns one paste into hundreds of files.
        for f in ALL {
            assert!(
                args_for(f).iter().any(|a| a == "--no-playlist"),
                "{f:?} lost --no-playlist"
            );
        }
    }

    #[test]
    fn print_is_always_paired_with_no_simulate() {
        // Without --no-simulate, --print makes yt-dlp simulate: the app would report
        // success while downloading nothing. Silent, and very confusing to debug.
        for f in ALL {
            let args = args_for(f);
            assert!(args.iter().any(|a| a == "--print"));
            assert!(args.iter().any(|a| a == "--no-simulate"), "{f:?}");
            assert!(args.iter().any(|a| a == "--progress"), "{f:?}");
        }
    }

    #[test]
    fn ffmpeg_location_is_always_passed() {
        for f in ALL {
            assert_eq!(
                value_after(&args_for(f), "--ffmpeg-location"),
                "/tmp/ffmpeg"
            );
        }
    }

    #[test]
    fn output_goes_to_the_requested_folder() {
        for f in ALL {
            assert_eq!(value_after(&args_for(f), "--paths"), "home:/tmp/dl");
        }
    }

    #[test]
    fn progress_template_carries_every_field_the_parser_reads() {
        let template = value_after(&args_for(Format::BestQuality), "--progress-template");
        for field in [
            "downloaded_bytes",
            "total_bytes",
            "total_bytes_estimate",
            "speed",
            "eta",
        ] {
            assert!(
                template.contains(field),
                "template missing {field}: {template}"
            );
        }
        assert!(template.starts_with("download:"));
        assert!(template.contains(PROGRESS_MARKER));
    }

    #[test]
    fn filename_template_stays_human_readable() {
        // Not "%(title)s [%(id)s].%(ext)s" — a grandma should find "Cat Video.mp4",
        // not "Cat Video [dQw4w9WgXcQ].mp4".
        assert_eq!(
            value_after(&args_for(Format::BestQuality), "--output"),
            "%(title)s.%(ext)s"
        );
    }

    #[test]
    fn mp3_preset_requests_audio_extraction_at_best_quality() {
        let args = Format::AudioMp3.args();
        assert!(args.iter().any(|a| a == "--extract-audio"));
        assert_eq!(value_after(&args, "--audio-format"), "mp3");
        assert_eq!(value_after(&args, "--audio-quality"), "0");
    }

    #[test]
    fn best_quality_targets_mp4_as_the_spec_requires() {
        assert_eq!(
            value_after(&Format::BestQuality.args(), "--merge-output-format"),
            "mp4"
        );
    }

    #[test]
    fn best_available_places_no_container_limit_on_quality() {
        // mkv accepts any codec pair, so the "might be huge" preset never has to
        // re-encode down to fit a container.
        assert_eq!(
            value_after(&Format::BestAvailable.args(), "--merge-output-format"),
            "mkv"
        );
        assert_eq!(value_after(&Format::BestAvailable.args(), "-f"), "bv*+ba/b");
    }

    #[test]
    fn every_format_selector_has_a_fallback() {
        // A selector with no '/' fails hard on any site that lacks the exact combo.
        for f in ALL {
            let selector = value_after(&f.args(), "-f");
            assert!(
                selector.contains('/'),
                "{f:?} selector has no fallback: {selector}"
            );
        }
    }

    #[test]
    fn format_ids_match_the_frontend_contract() {
        // These strings cross the IPC boundary and are duplicated in FormatSelector.jsx.
        // If serde's naming ever changes, the UI silently stops selecting formats.
        let json = |f: Format| serde_json::to_string(&f).unwrap();
        assert_eq!(json(Format::BestQuality), "\"best_quality\"");
        assert_eq!(json(Format::AudioMp3), "\"audio_mp3\"");
        assert_eq!(json(Format::BestAvailable), "\"best_available\"");
    }
}
