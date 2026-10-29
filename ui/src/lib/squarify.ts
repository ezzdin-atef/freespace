// Port of ui_kit::squarify: lays out values as rectangles filling `rect`, keeping them close to square.

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export function squarify(values: number[], rect: Rect): Rect[] {
  const total = values.reduce((a, b) => a + b, 0);
  if (total <= 0 || rect.w * rect.h <= 0) return values.map(() => ({ x: rect.x, y: rect.y, w: 0, h: 0 }));
  const area = rect.w * rect.h;
  const scaled = values.map((v) => (v / total) * area);
  // Worst aspect ratio of a row laid along a side of length `side`.
  const worst = (row: number[], side: number) => {
    const s = row.reduce((a, b) => a + b, 0);
    const mx = Math.max(...row);
    const mn = Math.min(...row);
    return Math.max((side * side * mx) / (s * s), (s * s) / (side * side * mn));
  };

  const out: Rect[] = [];
  const free = { ...rect };
  let i = 0;
  while (i < scaled.length) {
    const side = Math.min(free.w, free.h);
    let j = i + 1;
    while (j < scaled.length && worst(scaled.slice(i, j + 1), side) <= worst(scaled.slice(i, j), side)) j++;
    const row = scaled.slice(i, j);
    const sum = row.reduce((a, b) => a + b, 0);
    if (free.w >= free.h) {
      const w = sum / free.h;
      let y = free.y;
      for (const v of row) {
        const h = (v / sum) * free.h;
        out.push({ x: free.x, y, w, h });
        y += h;
      }
      free.x += w;
      free.w -= w;
    } else {
      const h = sum / free.w;
      let x = free.x;
      for (const v of row) {
        const w = (v / sum) * free.w;
        out.push({ x, y: free.y, w, h });
        x += w;
      }
      free.y += h;
      free.h -= h;
    }
    i = j;
  }
  return out;
}
