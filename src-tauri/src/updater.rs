//! App self-update (via Tauri's updater plugin) and the schedule for yt-dlp's
//! background refresh.
//!
//! These are two separate mechanisms and it is worth keeping them straight:
//!
//!   * The **app** updates through `tauri-plugin-updater`, which checks a signed
//!     `latest.json` and requires the user to say yes. Signed, visible, consented.
//!   * **yt-dlp** updates itself silently in the background, because it breaks
//!     whenever YouTube changes something and asking a non-technical user to approve
//!     a component update they have never heard of is not a real choice.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

use video_downloader_core::{AppError, Result};

/// How often to refresh yt-dlp. Daily is a deliberate middle ground: yt-dlp ships
/// fixes several times a month, and a stale copy is the single most common cause of
/// "this video won't download" — but checking on every launch would waste bandwidth
/// for someone who opens the app five times in an afternoon.
const YTDLP_CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60 * 24);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CheckState {
    last_ytdlp_check_unix: u64,
}

fn state_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    let dir = app.path().app_data_dir().ok()?;
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("update-state.json"))
}

fn read_state(app: &AppHandle) -> CheckState {
    state_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn ytdlp_check_is_due(app: &AppHandle) -> bool {
    let last = read_state(app).last_ytdlp_check_unix;
    // A zero timestamp means we have never checked; a timestamp in the future means
    // the system clock moved backwards. Both should trigger a check rather than
    // wedging the app into never updating again.
    last == 0
        || now_unix().saturating_sub(last) >= YTDLP_CHECK_INTERVAL.as_secs()
        || last > now_unix()
}

pub fn record_ytdlp_check(app: &AppHandle) {
    let state = CheckState {
        last_ytdlp_check_unix: now_unix(),
    };
    if let (Some(path), Ok(json)) = (state_path(app), serde_json::to_string_pretty(&state)) {
        let _ = std::fs::write(path, json);
    }
}

// ---------------------------------------------------------------------------
// App updates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub version: Option<String>,
    pub release_notes: Option<String>,
}

/// Spec command #4. Returns whether a newer app version exists.
///
/// Failure is reported as "no update available" rather than as an error: the update
/// server being down is our problem, not something to put in front of the user in the
/// middle of downloading a video.
#[tauri::command]
pub async fn check_app_updates(app: AppHandle) -> Result<UpdateInfo> {
    let updater = app
        .updater()
        .map_err(|e| AppError::Network(format!("updater unavailable: {e}")))?;

    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateInfo {
            available: true,
            version: Some(update.version.clone()),
            release_notes: update.body.clone(),
        }),
        Ok(None) => Ok(UpdateInfo {
            available: false,
            version: None,
            release_notes: None,
        }),
        Err(e) => {
            eprintln!("app update check failed: {e}");
            Ok(UpdateInfo {
                available: false,
                version: None,
                release_notes: None,
            })
        }
    }
}

/// Downloads and installs the pending update, then restarts.
///
/// The signature check is enforced by the plugin against the public key in
/// `tauri.conf.json` — an unsigned or wrongly-signed bundle is rejected before it is
/// ever executed. That is what makes silent-ish updating safe.
#[tauri::command]
pub async fn install_app_update(app: AppHandle) -> Result<()> {
    let updater = app
        .updater()
        .map_err(|e| AppError::Network(format!("updater unavailable: {e}")))?;

    let update = updater
        .check()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?
        .ok_or_else(|| AppError::Network("no update available".into()))?;

    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|e| AppError::Network(format!("could not install the update: {e}")))?;

    app.restart();
}
