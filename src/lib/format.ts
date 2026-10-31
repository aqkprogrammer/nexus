const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"];

/** Decimal units, matching how Finder and macOS report storage. */
export function bytes(value: number | null | undefined, digits = 1): string {
  if (value == null || !Number.isFinite(value)) return "Unavailable";
  if (value < 1000) return `${Math.round(value)} B`;
  const exponent = Math.min(Math.floor(Math.log10(value) / 3), UNITS.length - 1);
  const scaled = value / 1000 ** exponent;
  return `${scaled.toFixed(scaled >= 100 ? 0 : digits)} ${UNITS[exponent]}`;
}

/** Binary units, which is how Apple rates RAM and how Activity Monitor reports it. */
export function memory(value: number | null | undefined, digits = 1): string {
  if (value == null || !Number.isFinite(value)) return "Unavailable";
  const gb = value / 1024 ** 3;
  if (gb >= 1) return `${gb.toFixed(digits)} GB`;
  return `${Math.round(value / 1024 ** 2)} MB`;
}

export function splitBytes(value: number | null | undefined): [string, string] {
  const text = bytes(value);
  const [number, unit] = text.split(" ");
  return [number, unit ?? ""];
}

export function rate(value: number | null | undefined): string {
  return value == null ? "Unavailable" : `${bytes(value)}/s`;
}

export function percent(value: number | null | undefined, digits = 0): string {
  return value == null ? "Unavailable" : `${value.toFixed(digits)}%`;
}

export function count(value: number | null | undefined): string {
  return value == null ? "Unavailable" : value.toLocaleString("en-US");
}

export function duration(totalSeconds: number): string {
  const days = Math.floor(totalSeconds / 86_400);
  const hours = Math.floor((totalSeconds % 86_400) / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}

export function ago(timestampMs: number, now = Date.now()): string {
  const seconds = Math.max(0, Math.round((now - timestampMs) / 1000));
  if (seconds < 5) return "just now";
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86_400) return `${Math.floor(seconds / 3600)}h ago`;
  return `${Math.floor(seconds / 86_400)}d ago`;
}

export function date(timestampMs: number | null | undefined): string {
  if (!timestampMs) return "Unknown";
  return new Date(timestampMs).toLocaleDateString("en-US", { year: "numeric", month: "short", day: "numeric" });
}

export function dateFromSeconds(seconds: number | null | undefined): string {
  return seconds ? date(seconds * 1000) : "Unknown";
}

export function dateTime(timestampMs: number): string {
  return new Date(timestampMs).toLocaleString("en-US", {
    month: "short", day: "numeric", hour: "numeric", minute: "2-digit",
  });
}

export function clock(timestampMs: number): string {
  return new Date(timestampMs).toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });
}

/** Whole months between a Unix-seconds timestamp and now. */
export function ageMonths(seconds: number, now = Date.now()): number {
  return Math.floor((now / 1000 - seconds) / (30 * 86_400));
}

export function shortPath(path: string, home?: string): string {
  return home && path.startsWith(home) ? `~${path.slice(home.length)}` : path;
}

export function baseName(path: string): string {
  const trimmed = path.endsWith("/") ? path.slice(0, -1) : path;
  return trimmed.slice(trimmed.lastIndexOf("/") + 1) || trimmed;
}
