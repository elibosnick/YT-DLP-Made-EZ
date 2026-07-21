/**
 * The three format presets from the spec.
 *
 * Implemented as a native radio group inside a fieldset rather than a custom widget:
 * screen readers announce the group name and the "2 of 3" position for free, arrow
 * keys work as users expect, and there is no keyboard trap to get wrong.
 */

export const FORMATS = [
  {
    id: "best_quality",
    label: "Best Quality (Video + Audio)",
    hint: "Recommended. Saves an MP4 video file.",
  },
  {
    id: "audio_mp3",
    label: "Audio Only (MP3)",
    hint: "Just the sound, as an MP3 music file.",
  },
  {
    id: "best_available",
    label: "Best Available (Might be huge)",
    hint: "Highest possible quality. Can be a very large file.",
  },
];

export default function FormatSelector({ value, onChange, disabled }) {
  return (
    <fieldset className="format-selector" disabled={disabled}>
      <legend>Download as:</legend>

      {FORMATS.map((format) => (
        <label
          key={format.id}
          className={`format-option ${value === format.id ? "is-selected" : ""}`}
        >
          <input
            type="radio"
            name="format"
            value={format.id}
            checked={value === format.id}
            onChange={() => onChange(format.id)}
            disabled={disabled}
          />
          <span className="format-text">
            <span className="format-label">{format.label}</span>
            {/* The hint is associated with the label element that wraps the input, so
                it is read out as part of the option rather than as orphaned text. */}
            <span className="format-hint">{format.hint}</span>
          </span>
        </label>
      ))}
    </fieldset>
  );
}
