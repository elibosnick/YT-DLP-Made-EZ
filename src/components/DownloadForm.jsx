import { useEffect, useRef } from "react";
import FormatSelector from "./FormatSelector.jsx";

/**
 * URL field + format choice + the Download button.
 *
 * It is a real <form>, so pressing Enter anywhere in it submits — which is how most
 * people expect a single-field form to behave, and saves a tab-to-the-button step.
 */
export default function DownloadForm({
  url,
  onUrlChange,
  format,
  onFormatChange,
  onSubmit,
  busy,
  disabled,
}) {
  const inputRef = useRef(null);

  // Spec: focus the URL input on launch. The user's very first action is always
  // pasting, so the caret should already be waiting for them.
  useEffect(() => {
    if (!disabled) inputRef.current?.focus();
  }, [disabled]);

  function handleSubmit(event) {
    event.preventDefault();
    if (!busy && !disabled && url.trim()) onSubmit();
  }

  return (
    <form onSubmit={handleSubmit} noValidate>
      <label className="field-label" htmlFor="url-input">
        Paste video link:
      </label>
      <input
        id="url-input"
        ref={inputRef}
        type="text"
        className="url-input"
        value={url}
        onChange={(e) => onUrlChange(e.target.value)}
        placeholder="https://youtube.com/watch?v=..."
        disabled={busy || disabled}
        // Browsers "helpfully" capitalise and autocorrect pasted URLs on some
        // platforms, which silently corrupts them.
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck="false"
      />

      <FormatSelector value={format} onChange={onFormatChange} disabled={busy || disabled} />

      <button
        type="submit"
        className="button-primary"
        disabled={busy || disabled || !url.trim()}
      >
        {busy ? "Downloading…" : "Download"}
      </button>
    </form>
  );
}
