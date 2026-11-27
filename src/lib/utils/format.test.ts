import { describe, expect, it } from "vitest";
import { formatBytes, formatIsoDate, formatTimestamp } from "./format";

describe("format utilities", () => {
  it("formats byte sizes across units", () => {
    expect(formatBytes(null)).toBe("—");
    expect(formatBytes(undefined)).toBe("—");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB");
    expect(formatBytes(3 * 1024 * 1024 * 1024)).toBe("3.00 GB");
  });

  it("formats timestamps and ISO dates safely", () => {
    expect(formatTimestamp(0)).toBe("—");
    expect(formatIsoDate(null)).toBe("—");
    expect(formatIsoDate("not-a-date")).toBe("not-a-date");
    expect(formatIsoDate("2026-11-27T00:00:00Z")).not.toBe("—");
  });
});
