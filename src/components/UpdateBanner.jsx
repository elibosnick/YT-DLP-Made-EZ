/**
 * App update prompt.
 *
 * Deliberately a dismissible banner rather than a modal. A modal on launch trains
 * people to click things to make them go away, and this app's whole promise is that
 * it does not get in your way. "Later" is a real, remembered choice for this session.
 */
export default function UpdateBanner({ version, onInstall, onDismiss, installing }) {
  return (
    <div className="update-banner" role="status">
      <p className="update-text">
        A newer version of YT-DLP Made EZ is available{version ? ` (${version})` : ""}.
      </p>
      <div className="update-actions">
        <button
          type="button"
          className="button-secondary"
          onClick={onInstall}
          disabled={installing}
        >
          {installing ? "Installing…" : "Install now"}
        </button>
        <button
          type="button"
          className="button-link"
          onClick={onDismiss}
          disabled={installing}
        >
          Later
        </button>
      </div>
    </div>
  );
}
