// Ports of src/util.rs formatters, so sizes read the same as in the Rust side.

const UNITS = ["B", "KB", "MB", "GB", "TB"];

export function fmtSize(bytes: number): string {
  let v = bytes;
  let i = 0;
  // Roll over at 1000 so we print "0.98 MB" rather than "1003.6 KB".
  while (v >= 1000 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  return i === 0 ? `${bytes} B` : `${v.toFixed(1)} ${UNITS[i]}`;
}

export function fmtCount(n: number): string {
  return Math.trunc(n).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

/** "8,421", "52.3K", "1.20M" - short enough for stat cards and the top bar. */
export function fmtCompact(n: number): string {
  if (n < 10_000) return fmtCount(n);
  if (n < 1_000_000) return `${(n / 1e3).toFixed(1)}K`;
  return `${(n / 1e6).toFixed(2)}M`;
}

export function fmtSeconds(ms: number): string {
  return `${(ms / 1000).toFixed(1)}s`;
}

/** "today", "3 days ago", "11 months ago" for a time in seconds since 1970; `now` is for tests. */
export function fmtAgo(secs: number | null, now = Date.now() / 1000): string {
  if (secs === null) return "Unknown";
  const days = Math.floor((now - secs) / 86400);
  const ago = (n: number, unit: string) => `${n} ${unit}${n === 1 ? "" : "s"} ago`;
  if (days < 1) return "today";
  if (days < 14) return ago(days, "day");
  if (days < 60) return ago(Math.floor(days / 7), "week");
  if (days < 730) return ago(Math.floor(days / 30.44), "month");
  return ago(Math.floor(days / 365.25), "year");
}

/** "C:\" -> "C:", folders unchanged. */
export function driveLabel(root: string): string {
  return root.replace(/\\$/, "");
}
