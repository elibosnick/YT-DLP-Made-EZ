//! Local-only bridge for the optional Chrome helper.

use tauri::{AppHandle, Emitter};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

pub const MEDIA_EVENT: &str = "browser-media-url";

/// Listen only on loopback. The helper sends a small JSON body containing a media URL;
/// no cookies, page HTML, or credentials are accepted or stored here.
pub async fn run(app: AppHandle) -> std::io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", 32123)).await?;
    loop {
        let (mut socket, _) = listener.accept().await?;
        let app = app.clone();
        tokio::spawn(async move {
            let mut body = vec![0; 32 * 1024];
            let size = socket.read(&mut body).await.unwrap_or(0);
            let text = String::from_utf8_lossy(&body[..size]);
            let Some(json) = text.split("\r\n\r\n").nth(1) else {
                return;
            };
            let Ok(payload) = serde_json::from_str::<serde_json::Value>(json.trim()) else {
                return;
            };
            let Some(url) = payload.get("url").and_then(|v| v.as_str()) else {
                return;
            };
            if url.starts_with("https://") || url.starts_with("http://") {
                let _ = app.emit(MEDIA_EVENT, url.to_string());
            }
        });
    }
}
