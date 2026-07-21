// Suppress the extra console window on Windows in release builds. Without this the
// app opens a black terminal alongside the window, which looks broken to a
// non-technical user.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod process;
mod updater;
mod ytdlp;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(commands::DownloadLock::default())
        .invoke_handler(tauri::generate_handler![
            commands::download_video,
            commands::check_yt_dlp_version,
            commands::update_yt_dlp,
            commands::ensure_setup,
            commands::tools_ready,
            commands::reveal_file,
            updater::check_app_updates,
            updater::install_app_update,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // First-run setup and the periodic yt-dlp refresh both happen off the main
            // thread so the window paints immediately. The spec asks for a <2s launch;
            // blocking startup on a network fetch would blow that on a slow connection,
            // and on first run it would look like a hang.
            tauri::async_runtime::spawn(async move {
                if let Err(e) = ytdlp::ensure_tools(&handle).await {
                    // The frontend learns about this through `tools_ready`; log for
                    // developers rather than interrupting the user with a dialog.
                    eprintln!("setup failed: {e}");
                    return;
                }

                // Silent background update, per spec. Deliberately fire-and-forget:
                // a failed update check must never surface to the user, because they
                // still have a working yt-dlp and did not ask for this.
                if updater::ytdlp_check_is_due(&handle) {
                    match ytdlp::update_ytdlp(&handle).await {
                        Ok(msg) => {
                            eprintln!("yt-dlp: {msg}");
                            updater::record_ytdlp_check(&handle);
                        }
                        Err(e) => eprintln!("yt-dlp update skipped: {e}"),
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the application");
}
