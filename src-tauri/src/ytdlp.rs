//! Management of the two external binaries this app depends on: `yt-dlp` and `ffmpeg`.
//!
//! Both are fetched on first run into the app's data directory rather than bundled,
//! which keeps the installer small and lets yt-dlp update itself between app releases
//! (important: yt-dlp breaks often when sites change, and ships fixes several times a
//! month — far more often than we will ship the app).
//!
//! ffmpeg is required, not optional. "Audio Only (MP3)" needs it to transcode, and
//! "Best Quality" needs it to merge YouTube's separate video and audio streams.
//! Without it, two of our three format presets fail.
//!
//! Note on macOS: files written by this process do NOT get the `com.apple.quarantine`
//! extended attribute (that is applied by browsers and other LSFileQuarantineEnabled
//! apps), so the downloaded binaries execute without a Gatekeeper prompt.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use video_downloader_core::{AppError, Result};

/// Serializes helper-binary setup and updates across the whole process.
///
/// This matters because setup is triggered from two independent places on launch: the
/// backend `setup` hook in `main.rs`, and the frontend calling `ensure_setup` when the
/// window mounts. Without this lock they race — both download the same file, and when
/// one renames its temp file into place the other's rename finds nothing there
/// ("No such file or directory"). It also stops ffmpeg (~80MB) from being fetched twice.
///
/// The second caller to acquire the lock re-runs the existence/health checks, finds the
/// tools already present, and returns almost immediately.
static SETUP_LOCK: Mutex<()> = Mutex::const_new(());

/// Event emitted while first-run setup is downloading the helper binaries.
pub const SETUP_PROGRESS_EVENT: &str = "setup-progress";

#[derive(Debug, Clone, Serialize)]
pub struct SetupProgress {
    /// Human-readable, already grandma-friendly. Shown verbatim in the UI.
    pub message: String,
    /// 0-100 across the whole setup, not per-file.
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStatus {
    pub ytdlp_installed: bool,
    pub ytdlp_version: Option<String>,
    pub ffmpeg_installed: bool,
}

// ---------------------------------------------------------------------------
// Platform / URL table
// ---------------------------------------------------------------------------
//
// HANDOFF NOTE: these asset names are the one part of this file most likely to
// rot, because they are owned by upstream projects. `scripts/verify-binary-urls.sh`
// in the repo root HEAD-checks every URL below; CI runs it weekly so a rename
// surfaces as a failed build rather than as a broken first run for users.

/// Official yt-dlp release assets. yt-dlp ships a universal macOS binary that
/// covers both Intel and Apple Silicon, so there is no separate arm64 mac entry.
fn ytdlp_asset_name() -> Result<&'static str> {
    Ok(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", _) => "yt-dlp.exe",
        ("macos", _) => "yt-dlp_macos",
        ("linux", "aarch64") => "yt-dlp_linux_aarch64",
        ("linux", _) => "yt-dlp_linux",
        (os, arch) => return Err(AppError::UnsupportedPlatform(format!("{os}/{arch}"))),
    })
}

/// Static ffmpeg builds. We use the `eugeneware/ffmpeg-static` releases because it is
/// the only single source that publishes plain (non-archived) static binaries for all
/// five targets we care about, including darwin-arm64. That means no zip/tar extraction
/// code path and one URL shape to maintain.
fn ffmpeg_asset_name() -> Result<&'static str> {
    Ok(match (std::env::consts::OS, std::env::consts::ARCH) {
        // Note the missing `.exe` — upstream publishes the Windows build without an
        // extension. We save it locally as `ffmpeg.exe` regardless (see ffmpeg_path),
        // which is what actually matters for execution.
        ("windows", _) => "ffmpeg-win32-x64",
        ("macos", "aarch64") => "ffmpeg-darwin-arm64",
        ("macos", _) => "ffmpeg-darwin-x64",
        ("linux", "aarch64") => "ffmpeg-linux-arm64",
        ("linux", _) => "ffmpeg-linux-x64",
        (os, arch) => return Err(AppError::UnsupportedPlatform(format!("{os}/{arch}"))),
    })
}

const YTDLP_BASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
const FFMPEG_BASE: &str = "https://github.com/eugeneware/ffmpeg-static/releases/latest/download";

fn exe_suffix() -> &'static str {
    if cfg!(windows) {
        ".exe"
    } else {
        ""
    }
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

pub fn bin_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Io(format!("could not locate app data directory: {e}")))?
        .join("bin");
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::Io(format!("could not create {}: {e}", dir.display())))?;
    Ok(dir)
}

pub fn ytdlp_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(bin_dir(app)?.join(format!("yt-dlp{}", exe_suffix())))
}

pub fn ffmpeg_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(bin_dir(app)?.join(format!("ffmpeg{}", exe_suffix())))
}

// ---------------------------------------------------------------------------
// Download helpers
// ---------------------------------------------------------------------------

/// Streams `url` to `dest` and returns the SHA-256 of what was written, as lowercase hex.
///
/// Two things worth keeping if this is ever refactored:
///
///  * **Streamed, not buffered.** ffmpeg is around 80MB; `response.bytes()` would hold
///    the whole thing in memory at once, on top of the write buffer. The spec asks for
///    an app that runs on older machines without complaint, so we hash and write
///    incrementally and never hold more than one chunk.
///  * **Written to `.part`, then renamed.** Rename is atomic on every platform we
///    target, so an interrupted download can never leave a truncated binary sitting at
///    the real path looking perfectly valid.
async fn download_to(url: &str, dest: &Path) -> Result<String> {
    use futures_util::StreamExt;

    let response = reqwest::Client::builder()
        .user_agent("video-downloader-app")
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    if !response.status().is_success() {
        return Err(AppError::Network(format!(
            "{} returned HTTP {}",
            url,
            response.status()
        )));
    }

    let part = dest.with_extension("part");
    let mut file = tokio::fs::File::create(&part)
        .await
        .map_err(|e| AppError::Io(format!("could not write {}: {e}", part.display())))?;

    let mut hasher = Sha256::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Network(e.to_string()))?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|e| AppError::Io(e.to_string()))?;
    }

    file.flush()
        .await
        .map_err(|e| AppError::Io(e.to_string()))?;
    drop(file);

    tokio::fs::rename(&part, dest)
        .await
        .map_err(|e| AppError::Io(format!("could not finalize {}: {e}", dest.display())))?;

    make_executable(dest)?;
    Ok(hex_encode(&hasher.finalize()))
}

fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)
            .map_err(|e| AppError::Io(e.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).map_err(|e| AppError::Io(e.to_string()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// yt-dlp publishes a `SHA2-256SUMS` manifest with every release. We verify against it
/// because this binary is executed with user privileges; a corrupted or substituted
/// download is worth catching even though the transport is already HTTPS.
async fn verify_ytdlp_checksum(actual: &str, asset: &str) -> Result<()> {
    let manifest_url = format!("{YTDLP_BASE}/SHA2-256SUMS");
    let manifest = match reqwest::get(&manifest_url).await {
        Ok(r) if r.status().is_success() => match r.text().await {
            Ok(t) => t,
            Err(_) => return Ok(()), // manifest unreadable: fall through to the smoke test
        },
        // If the manifest is unavailable we do not hard-fail: the functional smoke test
        // still has to pass before we consider the tool installed, and hard-failing here
        // would mean a GitHub hiccup bricks first-run setup entirely.
        _ => return Ok(()),
    };

    let expected = manifest.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?;
        // The '*' prefix marks binary mode in the sha256sum format.
        (name.trim_start_matches('*') == asset).then(|| hash.to_ascii_lowercase())
    });

    let Some(expected) = expected else {
        return Ok(());
    };

    if actual != expected {
        return Err(AppError::ChecksumMismatch {
            expected,
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// The flag each tool accepts to print its version.
///
/// These differ, and getting it wrong is a silent disaster: ffmpeg's option parser
/// rejects the GNU-style `--version` with "Unrecognized option", so a shared
/// `--version` would make every ffmpeg install look broken and send the app into an
/// endless re-download loop. yt-dlp, being optparse-based, wants the long form.
const YTDLP_VERSION_FLAG: &str = "--version";
const FFMPEG_VERSION_FLAG: &str = "-version";

/// Fast readiness check for the UI's launch path: are both helper binaries present?
///
/// Deliberately does NOT run `--version` here. That functional check is worth doing —
/// but yt-dlp ships as a self-extracting binary that takes ~1 second to start on macOS,
/// and running it on every launch froze the window (the form stayed disabled until it
/// returned, which reads as a hang). So we only confirm the files exist here, which is
/// instant. The real smoke test happens once during first-run setup, and the background
/// `ensure_tools` task re-verifies on each launch and silently re-downloads a broken
/// binary — all off the interface's critical path.
pub async fn tools_ready(app: &AppHandle) -> bool {
    let (Ok(y), Ok(f)) = (ytdlp_path(app), ffmpeg_path(app)) else {
        return false;
    };
    y.exists() && f.exists()
}

/// Runs `<binary> <version flag>` to confirm the file is not merely present but
/// executable on this machine — catching partial downloads, a wrong-architecture
/// binary, and missing execute permission in a single step.
async fn smoke_test(path: &Path, version_flag: &str) -> Result<String> {
    let output = crate::process::command(path)
        .arg(version_flag)
        .output()
        .await
        .map_err(|e| AppError::BinaryUnusable(format!("{}: {e}", path.display())))?;

    if !output.status.success() {
        return Err(AppError::BinaryUnusable(format!(
            "{} exited with {}",
            path.display(),
            output.status
        )));
    }

    // ffmpeg prints a banner across many lines; the first is the version line.
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string())
}

/// First-run setup. Idempotent: re-running skips anything already working.
pub async fn ensure_tools(app: &AppHandle) -> Result<()> {
    // Serialize against the other setup trigger (see SETUP_LOCK). Held for the whole
    // download so a concurrent caller waits, then sails through the checks below.
    let _setup_guard = SETUP_LOCK.lock().await;

    let ytdlp = ytdlp_path(app)?;
    let ffmpeg = ffmpeg_path(app)?;

    if !ytdlp.exists() || smoke_test(&ytdlp, YTDLP_VERSION_FLAG).await.is_err() {
        emit_setup(app, "Getting the video downloader ready…", 10);
        let asset = ytdlp_asset_name()?;
        let digest = download_to(&format!("{YTDLP_BASE}/{asset}"), &ytdlp).await?;
        emit_setup(app, "Checking the download is genuine…", 40);
        verify_ytdlp_checksum(&digest, asset).await?;
        smoke_test(&ytdlp, YTDLP_VERSION_FLAG).await?;
    }

    if !ffmpeg.exists() || smoke_test(&ffmpeg, FFMPEG_VERSION_FLAG).await.is_err() {
        // Worth its own message: this is the big one (~80MB) and the step most likely
        // to make a user on a slow connection think the app has frozen.
        emit_setup(app, "Getting the audio converter ready…", 55);
        let asset = ffmpeg_asset_name()?;
        download_to(&format!("{FFMPEG_BASE}/{asset}"), &ffmpeg).await?;
        smoke_test(&ffmpeg, FFMPEG_VERSION_FLAG).await?;
    }

    emit_setup(app, "All set!", 100);
    Ok(())
}

fn emit_setup(app: &AppHandle, message: &str, percent: u8) {
    let _ = app.emit(
        SETUP_PROGRESS_EVENT,
        SetupProgress {
            message: message.to_string(),
            percent,
        },
    );
}

pub async fn status(app: &AppHandle) -> ToolStatus {
    let ytdlp = ytdlp_path(app).ok();
    let ffmpeg = ffmpeg_path(app).ok();

    let ytdlp_version = match &ytdlp {
        Some(p) if p.exists() => smoke_test(p, YTDLP_VERSION_FLAG).await.ok(),
        _ => None,
    };

    ToolStatus {
        ytdlp_installed: ytdlp_version.is_some(),
        ytdlp_version,
        ffmpeg_installed: match &ffmpeg {
            Some(p) => p.exists() && smoke_test(p, FFMPEG_VERSION_FLAG).await.is_ok(),
            None => false,
        },
    }
}

/// Silent background update of yt-dlp only.
///
/// We deliberately do not touch ffmpeg here: it is a large download, its interface is
/// stable, and re-fetching it on a schedule would burn users' bandwidth for no benefit.
/// yt-dlp is the piece that goes stale.
///
/// Failure is intentionally swallowed by the caller — if the update check fails the
/// user still has a working older yt-dlp, and interrupting them with an error about a
/// background task they did not ask for would be worse than staying quiet.
pub async fn update_ytdlp(app: &AppHandle) -> Result<String> {
    // Same lock as setup: never let a background update's download+rename overlap with a
    // setup download of the same file.
    let _setup_guard = SETUP_LOCK.lock().await;

    let path = ytdlp_path(app)?;
    let before = smoke_test(&path, YTDLP_VERSION_FLAG)
        .await
        .unwrap_or_default();

    let asset = ytdlp_asset_name()?;
    let digest = download_to(&format!("{YTDLP_BASE}/{asset}"), &path).await?;
    verify_ytdlp_checksum(&digest, asset).await?;

    let after = smoke_test(&path, YTDLP_VERSION_FLAG).await?;
    if before == after {
        Ok(format!("Already up to date ({after})"))
    } else {
        Ok(format!("Updated {before} -> {after}"))
    }
}
