import type { Category, CleanerReport, DuplicateReport, ScanSummary, VolumeSnapshot } from "./types";

export interface OptimizationInput {
  volume: VolumeSnapshot | null;
  storage: ScanSummary | null;
  cleaner: CleanerReport | null;
  duplicates: DuplicateReport | null;
}

/**
 * NEXUS's storage optimization score, 0–100. Starts from free space and subtracts for
 * reclaimable waste already measured. Returns null without disk data rather than guessing.
 */
export function optimizationScore({ volume, storage, cleaner, duplicates }: OptimizationInput): number | null {
  if (!volume || volume.total === 0) return null;
  const free = (volume.available / volume.total) * 100;
  const base = free >= 25 ? 100 : free >= 15 ? 75 + (free - 15) * 2.5 : free >= 5 ? 40 + (free - 5) * 3.5 : free * 8;
  const waste =
    (cleaner?.recommended ?? 0) +
    (cleaner?.trash.total ?? 0) +
    (duplicates?.recoverable ?? 0) +
    (storage?.old.downloadsBytes ?? 0);
  const wasteShare = (waste / volume.total) * 100;
  return Math.round(Math.max(0, Math.min(100, base - wasteShare * 1.5)));
}

export interface StorageSegment { id: Category | "system" | "free"; label: string; bytes: number }

export const CATEGORY_LABEL: Record<Category, string> = {
  applications: "Applications",
  documents: "Documents",
  downloads: "Downloads",
  media: "Photos & media",
  developer: "Developer files",
  caches: "Cache files",
  other: "Other files",
};

/** Colours are bound to the entity and never reassigned by rank. Validated for CVD on dark. */
export const CATEGORY_COLOR: Record<Category, string> = {
  applications: "#08a0bc",
  documents: "#e4569f",
  downloads: "#3f7ff0",
  media: "#b0851a",
  developer: "#9a68f0",
  caches: "#1b9d74",
  other: "#d95a3a",
};

export const SYSTEM_COLOR = "#46506a";

/**
 * Splits used disk space into scanned categories plus the remainder. The remainder is
 * labelled for what it is: bytes NEXUS did not measure, not a claim about their contents.
 */
export function storageSegments(volume: VolumeSnapshot, summary: ScanSummary): StorageSegment[] {
  const segments: StorageSegment[] = summary.categories
    .filter((c) => c.bytes > 0)
    .sort((a, b) => b.bytes - a.bytes)
    .map((c) => ({ id: c.category, label: CATEGORY_LABEL[c.category], bytes: c.bytes }));
  const scanned = segments.reduce((sum, s) => sum + s.bytes, 0);
  const remainder = Math.max(0, volume.used - scanned);
  if (remainder > 0) segments.push({ id: "system", label: "System & unscanned", bytes: remainder });
  return segments;
}
