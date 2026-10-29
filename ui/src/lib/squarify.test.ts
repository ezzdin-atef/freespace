import { expect, test } from "vitest";
import { squarify } from "./squarify";

// Same case as the Rust test it replaces (ui_kit::tests::squarify_fills_the_rect).
test("squarify fills the rect", () => {
  const r = { x: 0, y: 0, w: 600, h: 200 };
  const vals = [301, 36.2, 29.3, 21.4, 9.5, 8.4, 8.2];
  const out = squarify(vals, r);
  expect(out).toHaveLength(vals.length);
  const area = out.reduce((n, b) => n + b.w * b.h, 0);
  expect(Math.abs(area - r.w * r.h)).toBeLessThan(1);
  for (const b of out) {
    expect(b.x).toBeGreaterThanOrEqual(-0.01);
    expect(b.y).toBeGreaterThanOrEqual(-0.01);
    expect(b.x + b.w).toBeLessThanOrEqual(r.w + 0.01);
    expect(b.y + b.h).toBeLessThanOrEqual(r.h + 0.01);
  }
});

test("squarify handles empty input", () => {
  expect(squarify([0, 0], { x: 0, y: 0, w: 10, h: 10 }).every((b) => b.w === 0)).toBe(true);
});
