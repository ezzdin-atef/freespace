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
