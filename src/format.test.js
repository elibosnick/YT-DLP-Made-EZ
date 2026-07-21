import { describe, expect, it } from "vitest";
import { formatBytes, formatEta, formatSpeed, progressSummary } from "./format.js";

describe("formatBytes", () => {
  it("uses plain words for tiny sizes", () => {
    expect(formatBytes(512)).toBe("512 bytes");
  });

  it("scales through the units", () => {
    expect(formatBytes(1500)).toBe("1.5 KB");
    expect(formatBytes(4_200_000)).toBe("4.2 MB");
    expect(formatBytes(2_500_000_000)).toBe("2.5 GB");
  });

  it("drops a trailing .0 rather than showing '45.0 MB'", () => {
    expect(formatBytes(45_000_000)).toBe("45 MB");
  });

  it("keeps one decimal so a large download visibly moves", () => {
    expect(formatBytes(12_400_000)).toBe("12.4 MB");
  });

  it("returns empty string rather than NaN for missing input", () => {
    expect(formatBytes(null)).toBe("");
    expect(formatBytes(undefined)).toBe("");
    expect(formatBytes(-1)).toBe("");
  });
});

describe("formatEta", () => {
  it("avoids false precision", () => {
    // The point of these strings is that they stay true as the estimate wobbles.
    expect(formatEta(5)).toBe("almost done");
    expect(formatEta(90)).toBe("about 2 minutes left");
    expect(formatEta(7200)).toBe("about 2 hours left");
  });

  it("uses the singular where it reads better", () => {
    expect(formatEta(61)).toBe("about a minute left");
    expect(formatEta(3500)).toBe("about an hour left");
  });

  it("gets vaguer as the estimate gets less trustworthy", () => {
    // Under 45 min we still give minutes; past that the number is noise.
    expect(formatEta(600)).toBe("about 10 minutes left");
    expect(formatEta(2400)).toBe("about 40 minutes left");
    expect(formatEta(3000)).toBe("about an hour left");
  });

  it("handles unknown ETA", () => {
    expect(formatEta(null)).toBe("");
    expect(formatEta(undefined)).toBe("");
  });
});

describe("formatSpeed", () => {
  it("appends a rate", () => {
    expect(formatSpeed(1_200_000)).toBe("1.2 MB/s");
  });

  it("omits a zero or unknown speed entirely", () => {
    expect(formatSpeed(0)).toBe("");
    expect(formatSpeed(null)).toBe("");
  });
});

describe("progressSummary", () => {
  it("joins the parts it has", () => {
    expect(
      progressSummary({
        downloadedBytes: 12_400_000,
        totalBytes: 68_000_000,
        speedBytesPerSec: 1_200_000,
        etaSeconds: 45,
      })
    ).toBe("12.4 MB of 68 MB · 1.2 MB/s · about 50 seconds left");
  });

  it("degrades gracefully when the size is unknown", () => {
    // Common on sites that stream without Content-Length.
    expect(progressSummary({ downloadedBytes: 5_000_000 })).toBe("5 MB");
  });

  it("produces nothing rather than stray separators when it knows nothing", () => {
    expect(progressSummary({})).toBe("");
  });
});
