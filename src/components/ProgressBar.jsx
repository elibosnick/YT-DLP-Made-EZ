import { progressSummary } from "../format.js";

/**
 * Download progress.
 *
 * Two states worth calling out:
 *
 *  - `indeterminate` — we are downloading but the site never told us the file size, so
 *    a percentage would be a lie. Shows a moving stripe instead of a fake number.
 *  - `converting` — bytes are in and ffmpeg is merging or transcoding. This reports no
 *    progress of its own, so without a distinct state the bar sits at 100% looking
 *    frozen, which reads as a crash.
 */
export default function ProgressBar({ percent, phase, downloadedBytes, totalBytes, speedBytesPerSec, etaSeconds }) {
  const converting = phase === "converting";
  const indeterminate = converting || !totalBytes;
  const clamped = Math.max(0, Math.min(100, percent ?? 0));

  const summary = converting
    ? "Finishing up — this takes a moment for large videos."
    : progressSummary({ downloadedBytes, totalBytes, speedBytesPerSec, etaSeconds });

  return (
    <div className="progress-wrap">
      <div
        className={`progress-track ${indeterminate ? "is-indeterminate" : ""}`}
        role="progressbar"
        aria-label={converting ? "Finishing up" : "Download progress"}
        // Omitting aria-valuenow is what tells assistive tech the value is unknown.
        // Reporting a made-up 0 would be actively misleading.
        {...(indeterminate
          ? {}
          : { "aria-valuenow": Math.round(clamped), "aria-valuemin": 0, "aria-valuemax": 100 })}
      >
        <div
          className="progress-fill"
          style={{ width: indeterminate ? "100%" : `${clamped}%` }}
        />
      </div>

      <p className="progress-summary">
        {!indeterminate && <strong>{Math.round(clamped)}%</strong>}
        {!indeterminate && summary && " · "}
        {summary}
      </p>
    </div>
  );
}
