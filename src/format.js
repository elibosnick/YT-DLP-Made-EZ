/**
 * Human-readable formatting helpers.
 *
 * These are separated from the components so they can be unit tested directly — the
 * wording here is user-facing and easy to regress.
 */

/** "4.2 MB" — decimal units, because that is what a file manager shows the user. */
export function formatBytes(bytes) {
  if (bytes == null || Number.isNaN(bytes) || bytes < 0) return "";
  if (bytes < 1000) return `${bytes} bytes`;

  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1000;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }

  // One decimal place, with a trailing ".0" stripped. Keeping the decimal matters
  // during a download: on a 4 GB file an integer-only readout would appear frozen for
  // ten seconds at a time, which reads as a stall.
  const rounded = value.toFixed(1).replace(/\.0$/, "");
  return `${rounded} ${units[unit]}`;
}

/** "1.2 MB/s". Returns "" when the speed is unknown so the UI can omit it entirely. */
export function formatSpeed(bytesPerSecond) {
  if (!bytesPerSecond || bytesPerSecond <= 0) return "";
  return `${formatBytes(bytesPerSecond)}/s`;
}

/**
 * Deliberately vague at the top end. A precise "47 minutes left" from an estimate that
 * swings wildly is worse than "a few minutes" — it invites the user to trust a number
 * that is about to change.
 */
export function formatEta(seconds) {
  if (seconds == null || Number.isNaN(seconds) || seconds < 0) return "";
  if (seconds < 10) return "almost done";
  if (seconds < 60) return `about ${Math.round(seconds / 10) * 10} seconds left`;

  // The ladder gets coarser as the estimate gets less trustworthy. "58 minutes left"
  // implies a precision yt-dlp's ETA simply does not have at that range, and it will
  // be wrong by ten minutes either way — "about an hour" stays true.
  if (seconds < 2700) {
    const minutes = Math.round(seconds / 60);
    return minutes <= 1 ? "about a minute left" : `about ${minutes} minutes left`;
  }
  if (seconds < 5400) return "about an hour left";
  return `about ${Math.round(seconds / 3600)} hours left`;
}

/** The line under the progress bar, e.g. "12.4 MB of 68.0 MB · 1.2 MB/s · about a minute left". */
export function progressSummary({ downloadedBytes, totalBytes, speedBytesPerSec, etaSeconds }) {
  const parts = [];
  if (totalBytes) {
    parts.push(`${formatBytes(downloadedBytes)} of ${formatBytes(totalBytes)}`);
  } else if (downloadedBytes) {
    parts.push(formatBytes(downloadedBytes));
  }
  const speed = formatSpeed(speedBytesPerSec);
  if (speed) parts.push(speed);
  const eta = formatEta(etaSeconds);
  if (eta) parts.push(eta);
  return parts.join(" · ");
}
