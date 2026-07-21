import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import DownloadForm from "./components/DownloadForm.jsx";
import ProgressBar from "./components/ProgressBar.jsx";
import StatusMessage from "./components/StatusMessage.jsx";
import UpdateBanner from "./components/UpdateBanner.jsx";

/** Spec: clear the success message after 5 seconds. */
const SUCCESS_CLEAR_MS = 5000;

export default function App() {
  // Setup is a separate axis from downloading: the app can be mid-first-run-setup
  // while the user is already typing a URL, and both need to be represented.
  const [setupState, setSetupState] = useState("checking"); // checking | installing | ready | failed
  const [setupProgress, setSetupProgress] = useState(null);
  const [setupError, setSetupError] = useState(null);

  const [url, setUrl] = useState("");
  const [format, setFormat] = useState("best_quality");

  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState(null);
  const [result, setResult] = useState(null);
  const [error, setError] = useState(null);

  const [update, setUpdate] = useState(null);
  const [installingUpdate, setInstallingUpdate] = useState(false);

  const successTimer = useRef(null);

  // --- First-run setup -------------------------------------------------------

  const runSetup = useCallback(async () => {
    setSetupError(null);
    try {
      const ready = await invoke("tools_ready");
      if (ready) {
        setSetupState("ready");
        return;
      }
      setSetupState("installing");
      await invoke("ensure_setup");
      setSetupState("ready");
    } catch (e) {
      setSetupState("failed");
      setSetupError(e);
    }
  }, []);

  useEffect(() => {
    runSetup();
  }, [runSetup]);

  useEffect(() => {
    const unlisten = listen("setup-progress", (event) => setSetupProgress(event.payload));
    // listen() resolves to the unsubscribe function, so the cleanup has to await it.
    // Returning it directly would hand React a Promise and leak the listener.
    return () => {
      unlisten.then((fn) => fn()).catch(() => {});
    };
  }, []);

  // --- Download progress -----------------------------------------------------

  useEffect(() => {
    const unlisten = listen("download-progress", (event) => setProgress(event.payload));
    return () => {
      unlisten.then((fn) => fn()).catch(() => {});
    };
  }, []);

  // --- App updates -----------------------------------------------------------

  useEffect(() => {
    invoke("check_app_updates")
      .then((info) => {
        if (info?.available) setUpdate(info);
      })
      // A failed update check is our problem, not the user's. Stay silent.
      .catch(() => {});
  }, []);

  // Clear any pending timer on unmount so a fired callback cannot set state on an
  // unmounted component.
  useEffect(() => () => clearTimeout(successTimer.current), []);

  // --- Actions ---------------------------------------------------------------

  async function handleDownload() {
    clearTimeout(successTimer.current);
    setError(null);
    setResult(null);
    setProgress(null);
    setDownloading(true);

    try {
      const downloadResult = await invoke("download_video", { url, format });
      setResult(downloadResult);
      setProgress(null);
      // Spec: auto-clear the success message after 5 seconds.
      successTimer.current = setTimeout(() => setResult(null), SUCCESS_CLEAR_MS);
    } catch (e) {
      // The Rust side already translated this into { message, detail, retryable }.
      setError(e);
      setProgress(null);
    } finally {
      setDownloading(false);
    }
  }

  async function handleInstallUpdate() {
    setInstallingUpdate(true);
    try {
      await invoke("install_app_update");
      // On success the app restarts, so nothing after this runs.
    } catch {
      setInstallingUpdate(false);
      setUpdate(null);
    }
  }

  async function handleReveal() {
    if (result?.file_path) {
      try {
        await invoke("reveal_file", { path: result.file_path });
      } catch {
        // Opening the file manager is a convenience; failing to do so is not worth
        // replacing the success message with an error.
      }
    }
  }

  // --- Render ----------------------------------------------------------------

  const busy = downloading || setupState === "checking" || setupState === "installing";

  return (
    <main className="app">
      <h1 className="app-title">
        <span aria-hidden="true">📥</span> YT-DLP Made EZ
      </h1>

      {update && (
        <UpdateBanner
          version={update.version}
          installing={installingUpdate}
          onInstall={handleInstallUpdate}
          onDismiss={() => setUpdate(null)}
        />
      )}

      <DownloadForm
        url={url}
        onUrlChange={setUrl}
        format={format}
        onFormatChange={setFormat}
        onSubmit={handleDownload}
        busy={downloading}
        disabled={setupState !== "ready"}
      />

      <div className="status-area">
        {setupState === "installing" && (
          <>
            <p className="status status-info" role="status">
              {setupProgress?.message ?? "Getting things ready…"}
            </p>
            <ProgressBar percent={setupProgress?.percent ?? 0} totalBytes={100} />
            <p className="setup-note">This only happens the first time you open the app.</p>
          </>
        )}

        {setupState === "failed" && (
          <StatusMessage
            kind="error"
            message={setupError?.message ?? "Setup didn't finish. Check your internet connection."}
            detail={setupError?.detail}
            onRetry={runSetup}
          />
        )}

        {downloading && progress && (
          <ProgressBar
            percent={progress.percent}
            phase={progress.phase}
            downloadedBytes={progress.downloaded_bytes}
            totalBytes={progress.total_bytes}
            speedBytesPerSec={progress.speed_bytes_per_sec}
            etaSeconds={progress.eta_seconds}
          />
        )}

        {downloading && !progress && (
          <p className="status status-info" role="status">
            Starting…
          </p>
        )}

        {result && (
          <StatusMessage
            kind="success"
            message={`Your ${format === "audio_mp3" ? "audio" : "video"} is ready in Downloads!`}
            onReveal={handleReveal}
          />
        )}

        {error && (
          <StatusMessage
            kind="error"
            message={error.message}
            detail={error.detail}
            onRetry={error.retryable ? handleDownload : undefined}
          />
        )}

        {!busy && !result && !error && setupState === "ready" && (
          <p className="status status-info" role="status">
            Ready to download
          </p>
        )}
      </div>

      <footer className="privacy-note">
        <span aria-hidden="true">ⓘ</span> Downloads stay on your computer. Nothing is sent
        to us.
      </footer>
    </main>
  );
}
