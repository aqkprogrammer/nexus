import { describe, expect, it } from "vitest";
import { ago, ageMonths, baseName, bytes, count, duration, memory, percent, rate, shortPath } from "./format";
import { optimizationScore, storageSegments } from "./score";
import { squarify } from "./treemap";
import type { CleanerReport, ScanSummary, VolumeSnapshot } from "./types";

const volume = (total: number, available: number): VolumeSnapshot => ({
  mountPoint: "/", total, available, used: total - available,
});

describe("format", () => {
  it("uses decimal units like Finder", () => {
    expect(bytes(999)).toBe("999 B");
    expect(bytes(1500)).toBe("1.5 KB");
    expect(bytes(18_400_000_000)).toBe("18.4 GB");
    expect(bytes(245_000_000_000)).toBe("245 GB");
  });

  it("reports RAM in binary units so 16 GB reads as 16 GB", () => {
    expect(memory(17_179_869_184, 0)).toBe("16 GB");
    expect(memory(512 * 1024 ** 2)).toBe("512 MB");
  });

  it("says Unavailable instead of inventing a value", () => {
    for (const fn of [bytes, rate, percent, count, memory]) {
      expect(fn(null)).toBe("Unavailable");
      expect(fn(undefined)).toBe("Unavailable");
    }
  });

  it("formats durations and relative times", () => {
    expect(duration(90_000)).toBe("1d 1h");
    expect(duration(3_700)).toBe("1h 1m");
    expect(duration(120)).toBe("2m");
    expect(ago(1000, 1000)).toBe("just now");
    expect(ago(0, 7_200_000)).toBe("2h ago");
  });

  it("shortens paths under home only", () => {
    expect(shortPath("/Users/a/Downloads/x", "/Users/a")).toBe("~/Downloads/x");
    expect(shortPath("/Applications/x", "/Users/a")).toBe("/Applications/x");
    expect(baseName("/a/b/c.txt")).toBe("c.txt");
    expect(baseName("/a/b/")).toBe("b");
  });

  it("computes age in months", () => {
    const now = 1_700_000_000_000;
    expect(ageMonths(now / 1000 - 90 * 86_400, now)).toBe(3);
  });
});

describe("squarify", () => {
  const bounds = { x: 0, y: 0, w: 400, h: 300 };
  const items = [500, 300, 200, 100, 60, 40, 10].map((value, i) => ({ value, item: i }));
  const tiles = squarify(items, bounds);

  it("gives every tile an area proportional to its value", () => {
    const total = items.reduce((sum, i) => sum + i.value, 0);
    for (const tile of tiles) {
      const expected = (items[tile.item].value / total) * 120_000;
      expect(tile.w * tile.h).toBeCloseTo(expected, 3);
    }
  });

  it("stays inside the bounds and fills them", () => {
    let area = 0;
    for (const t of tiles) {
      expect(t.x).toBeGreaterThanOrEqual(-1e-6);
      expect(t.y).toBeGreaterThanOrEqual(-1e-6);
      expect(t.x + t.w).toBeLessThanOrEqual(400 + 1e-6);
      expect(t.y + t.h).toBeLessThanOrEqual(300 + 1e-6);
      area += t.w * t.h;
    }
    expect(area).toBeCloseTo(120_000, 3);
  });

  it("does not overlap tiles", () => {
    for (const a of tiles) {
      for (const b of tiles) {
        if (a === b) continue;
        const overlapX = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
        const overlapY = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
        expect(overlapX <= 1e-6 || overlapY <= 1e-6).toBe(true);
      }
    }
  });

  it("drops empty values and handles empty input", () => {
    expect(squarify([{ value: 0, item: 1 }], bounds)).toEqual([]);
    expect(squarify([], bounds)).toEqual([]);
    expect(squarify([{ value: 5, item: 1 }], { x: 0, y: 0, w: 0, h: 10 })).toEqual([]);
  });

  it("keeps tiles reasonably square", () => {
    const worst = Math.max(...tiles.map((t) => Math.max(t.w / t.h, t.h / t.w)));
    expect(worst).toBeLessThan(6);
  });
});

describe("optimizationScore", () => {
  const cleaner = (recommended: number): CleanerReport =>
    ({ recommended, trash: { total: 0 } }) as CleanerReport;

  it("returns null without disk data", () => {
    expect(optimizationScore({ volume: null, storage: null, cleaner: null, duplicates: null })).toBeNull();
  });

  it("is high with plenty of space and no measured waste", () => {
    expect(optimizationScore({ volume: volume(1000, 500), storage: null, cleaner: null, duplicates: null })).toBe(100);
  });

  it("falls as the disk fills", () => {
    const at = (free: number) =>
      optimizationScore({ volume: volume(100, free), storage: null, cleaner: null, duplicates: null })!;
    expect(at(20)).toBeGreaterThan(at(10));
    expect(at(10)).toBeGreaterThan(at(3));
  });

  it("drops when reclaimable waste is measured", () => {
    const clean = optimizationScore({ volume: volume(1000, 300), storage: null, cleaner: cleaner(0), duplicates: null })!;
    const messy = optimizationScore({ volume: volume(1000, 300), storage: null, cleaner: cleaner(100), duplicates: null })!;
    expect(messy).toBeLessThan(clean);
  });
});

describe("storageSegments", () => {
  const summary = {
    categories: [
      { category: "documents", bytes: 100 },
      { category: "media", bytes: 300 },
      { category: "other", bytes: 0 },
    ],
  } as ScanSummary;

  it("attributes the unmeasured remainder to System & unscanned", () => {
    const segments = storageSegments(volume(1000, 200), summary);
    expect(segments.map((s) => s.id)).toEqual(["media", "documents", "system"]);
    expect(segments.at(-1)!.bytes).toBe(400);
  });

  it("never reports a negative remainder", () => {
    const segments = storageSegments(volume(350, 100), summary);
    expect(segments.some((s) => s.id === "system")).toBe(false);
  });
});
