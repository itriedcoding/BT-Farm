import { describe, expect, it } from "vitest";
import {
  envToText,
  formatBytes,
  formatInterval,
  formatMb,
  formatRelative,
  localInputToRfc3339,
  parseResolution,
  textToEnv,
  truncate,
} from "./format";

describe("formatBytes", () => {
  it("scales units", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(999)).toBe("999 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(1024 * 1024 * 3)).toBe("3.0 MB");
  });
  it("handles missing data", () => {
    expect(formatBytes(null)).toBe("—");
  });
});

describe("formatMb", () => {
  it("switches to GB above 1024 MB", () => {
    expect(formatMb(512)).toBe("512 MB");
    expect(formatMb(4096)).toBe("4.0 GB");
    expect(formatMb(null)).toBe("—");
  });
});

describe("formatRelative", () => {
  const now = Date.parse("2026-01-01T12:00:00Z");
  it("describes recent times", () => {
    expect(formatRelative(null, now)).toBe("never");
    expect(formatRelative("2026-01-01T11:59:30Z", now)).toBe("30s ago");
    expect(formatRelative("2026-01-01T11:30:00Z", now)).toBe("30m ago");
    expect(formatRelative("2026-01-01T09:00:00Z", now)).toBe("3h ago");
  });
});

describe("parseResolution", () => {
  it("reads device screens", () => {
    expect(parseResolution("1080×2400")).toEqual({ width: 1080, height: 2400 });
    expect(parseResolution("something else")).toBeNull();
    expect(parseResolution(null)).toBeNull();
  });
});

describe("env helpers", () => {
  it("round trips", () => {
    const text = "API_URL=https://example.com\nDEBUG=1";
    expect(textToEnv(text)).toEqual({ API_URL: "https://example.com", DEBUG: "1" });
    expect(envToText(textToEnv(text))).toBe(text);
  });
  it("rejects malformed lines", () => {
    expect(() => textToEnv("nonsense")).toThrow();
    expect(() => textToEnv("1BAD=value")).toThrow();
  });
});

describe("scheduling helpers", () => {
  it("formats intervals", () => {
    expect(formatInterval(30, null)).toBe("Every 30 min");
    expect(formatInterval(120, null)).toBe("Every 2 hour(s)");
    expect(formatInterval(0, "09:30")).toBe("Daily at 09:30");
  });
  it("converts local input to RFC3339", () => {
    expect(localInputToRfc3339("")).toBeNull();
    expect(localInputToRfc3339("2026-03-04T05:06")).toMatch(/^2026-03-0[45]T\d\d:06:00\.000Z$/);
  });
});

describe("truncate", () => {
  it("adds an ellipsis only when needed", () => {
    expect(truncate("short", 10)).toBe("short");
    expect(truncate("abcdefghijklmno", 10)).toBe("abcdefghi…");
  });
});
