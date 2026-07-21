import { useState } from "react";

/**
 * The single place any status, success, or error text appears.
 *
 * Announcement policy: errors and successes use `role="alert"` (assertive) because the
 * user is waiting on that outcome and should hear it immediately. Routine status text
 * uses a polite live region so it does not interrupt them mid-sentence while typing.
 */
export default function StatusMessage({ kind, message, detail, onRetry, onReveal }) {
  const [showDetail, setShowDetail] = useState(false);

  if (!message) return null;

  const isError = kind === "error";
  const isSuccess = kind === "success";

  return (
    <div
      className={`status status-${kind}`}
      role={isError || isSuccess ? "alert" : "status"}
      aria-live={isError || isSuccess ? "assertive" : "polite"}
    >
      <p className="status-text">
        {isSuccess && <span aria-hidden="true">✓ </span>}
        {isError && <span aria-hidden="true">⚠ </span>}
        {message}
      </p>

      <div className="status-actions">
        {isSuccess && onReveal && (
          <button type="button" className="button-secondary" onClick={onReveal}>
            Show me the file
          </button>
        )}

        {isError && onRetry && (
          <button type="button" className="button-secondary" onClick={onRetry}>
            Try again?
          </button>
        )}

        {/* Collapsed by default. A non-technical user should never be shown a stack
            trace, but someone filing a GitHub issue needs something to paste. */}
        {isError && detail && (
          <button
            type="button"
            className="button-link"
            aria-expanded={showDetail}
            onClick={() => setShowDetail((v) => !v)}
          >
            {showDetail ? "Hide technical details" : "Show technical details"}
          </button>
        )}
      </div>

      {isError && detail && showDetail && (
        <pre className="status-detail" tabIndex={0}>
          {detail}
        </pre>
      )}
    </div>
  );
}
