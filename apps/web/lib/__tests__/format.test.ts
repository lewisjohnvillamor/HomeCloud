import { describe, expect, it } from "vitest";
import { formatBytes, formatDate, joinPath, parentOf } from "@/lib/format";

describe("formatBytes", () => {
  it("uses plain bytes below a kilobyte", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(999)).toBe("999 B");
  });

  it("steps up through decimal units", () => {
    expect(formatBytes(1000)).toBe("1.0 kB");
    expect(formatBytes(1_500_000)).toBe("1.5 MB");
    expect(formatBytes(4_000_000_000)).toBe("4.0 GB");
  });

  it("refuses to invent a size it does not have", () => {
    expect(formatBytes(Number.NaN)).toBe("—");
    expect(formatBytes(-1)).toBe("—");
  });
});

describe("formatDate", () => {
  it("renders a date it can parse", () => {
    expect(formatDate("2026-03-04T10:00:00Z")).toContain("2026");
  });

  it("shows a placeholder rather than `Invalid Date`", () => {
    expect(formatDate(null)).toBe("—");
    expect(formatDate("not a date")).toBe("—");
  });
});

describe("path helpers", () => {
  it("joins names onto a folder", () => {
    expect(joinPath("", "notes.txt")).toBe("notes.txt");
    expect(joinPath("photos/2026", "beach.jpg")).toBe("photos/2026/beach.jpg");
  });

  it("finds the containing folder", () => {
    expect(parentOf("notes.txt")).toBe("");
    expect(parentOf("photos/2026/beach.jpg")).toBe("photos/2026");
  });
});
