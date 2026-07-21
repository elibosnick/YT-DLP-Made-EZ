//! Tauri commands exposed to the React frontend.
//!
//! This is a thin adapter. All the logic that is worth testing — URL validation, the
//! yt-dlp argument list, progress parsing, error translation — lives in the
//! `video-downloader-core` crate, which has no Tauri dependency and so can be tested
//! without a GUI toolchain. What remains here is process orchestration.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

use video_downloader_core::{
    build_ytdlp_args, is_conversion_line, parse_filepath_line, parse_progress_line, validate_url,
    AppError, DownloadProgress, Format, Result,
};

use crate::ytdlp;

pub const PROGRESS_EVENT: &str = "download-progress";

/// Guards against two downloads running at once. The UI disables the button, but a
/// double-fired event or a key repeat should not be able to spawn a second yt-dlp
/// writing to the same output file.
#[derive(Default)]
pub struct DownloadLock(pub Arc<Mutex<()>>);

#[derive(Debug, Clone, Serialize)]
pub struct DownloadResult {
    pub file_path: String,
    pub file_name: String,
    /// The containing directory, so the UI can offer "Show me the file" without
    /// having to re-derive it from the path.
    pub folder: String,
}

#[tauri::command]
pub async fn download_video(
    app: AppHandle,
    lock: State<'_, DownloadLock>,
    url: String,
    format: Format,
) -> Result<DownloadResult> {
    // Validate before taking the lock, so a typo gives instant feedback.
    let url = validate_url(&url)?;

    let guard = lock.0.clone();
    let _held = guard
        .try_lock()
        .map_err(|_| AppError::DownloadFailed("a download is already running".into()))?;

    if !ytdlp::tools_ready(&app).await {
        return Err(AppError::NotReady);
    }

    let ytdlp_bin = ytdlp::ytdlp_path(&app)?;
    let ffmpeg_bin = ytdlp::ffmpeg_path(&app)?;
    let downloads = app
        .path()
        .download_dir()
        .map_err(|e| AppError::Io(format!("could not find your Downloads folder: {e}")))?;

    let mut cmd = crate::process::command(&ytdlp_bin);
    cmd.args(build_ytdlp_args(format, &ffmpeg_bin, &downloads))
        // `--` terminates option parsing. Without it, a URL beginning with a dash
        // would be interpreted as a flag.
        .arg("--")
        .arg(&url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::BinaryUnusable(format!("could not start the downloader: {e}")))?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");

    // stderr must be drained concurrently with stdout. If we read only stdout and
    // yt-dlp writes enough to stderr to fill the OS pipe buffer (~64KB, easily reached
    // by warnings on a long download), the child blocks on write and the app hangs
    // with no error — one of the nastier bugs to diagnose after the fact.
    let stderr_task = tokio::spawn(async move {
        let mut buf = String::new();
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            buf.push_str(&line);
            buf.push('\n');
        }
        buf
    });

    let mut final_path: Option<PathBuf> = None;
    let mut phase: &'static str = "downloading";
    let mut stdout_tail = String::new();

    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(mut progress) = parse_progress_line(&line) {
            progress.phase = phase;
            let _ = app.emit(PROGRESS_EVENT, &progress);
            continue;
        }

        if let Some(path) = parse_filepath_line(&line) {
            final_path = Some(path);
            continue;
        }

        if phase == "downloading" && is_conversion_line(&line) {
            phase = "converting";
            let _ = app.emit(PROGRESS_EVENT, DownloadProgress::converting());
        }

        // Keep a bounded tail of stdout: some yt-dlp errors land here rather than on
        // stderr, and it makes the bug-report detail considerably more useful.
        if stdout_tail.len() < 4000 {
            stdout_tail.push_str(&line);
            stdout_tail.push('\n');
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| AppError::DownloadFailed(e.to_string()))?;
    let stderr_output = stderr_task.await.unwrap_or_default();

    if !status.success() {
        return Err(AppError::DownloadFailed(format!(
            "{stderr_output}\n{stdout_tail}"
        )));
    }

    let path = final_path.ok_or_else(|| {
        AppError::DownloadFailed(format!(
            "the download finished but the file could not be located\n{stderr_output}"
        ))
    })?;

    Ok(DownloadResult {
        file_name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "your video".to_string()),
        folder: path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        file_path: path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn check_yt_dlp_version(app: AppHandle) -> Result<ytdlp::ToolStatus> {
    Ok(ytdlp::status(&app).await)
}

#[tauri::command]
pub async fn update_yt_dlp(app: AppHandle) -> Result<String> {
    ytdlp::update_ytdlp(&app).await
}

/// Runs first-run setup. Idempotent — returns immediately if everything is in place.
#[tauri::command]
pub async fn ensure_setup(app: AppHandle) -> Result<()> {
    ytdlp::ensure_tools(&app).await
}

#[tauri::command]
pub async fn tools_ready(app: AppHandle) -> bool {
    ytdlp::tools_ready(&app).await
}

/// Opens the system file manager with the finished file selected.
#[tauri::command]
pub async fn reveal_file(app: AppHandle, path: String) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| AppError::Io(e.to_string()))
}
