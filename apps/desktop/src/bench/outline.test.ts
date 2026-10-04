import { describe, expect, it } from "vitest";
import {
  CURVE,
  decodeHit,
  innerBox,
  LINE,
  makeGlyph,
  makeNestedGlyph,
  OFF_CURVE,
} from "./outline";

describe("makeGlyph", () => {
  it("builds the 5,000-point benchmark glyph by default", () => {
    const glyph = makeGlyph();
    expect(glyph.flags.length).toBe(5000);
    expect(glyph.coords.length).toBe(10000);
    expect(glyph.contourEnds.length).toBe(50);
    expect(glyph.contourEnds[49]).toBe(4999);
    expect(glyph.coords.every(Number.isFinite)).toBe(true);
  });

  it("starts each contour with a line point, then off, off, curve", () => {
    const { flags, contourEnds } = makeGlyph(2, 7);
    expect([...flags]).toEqual([
      LINE,
      OFF_CURVE,
      OFF_CURVE,
      CURVE,
      OFF_CURVE,
      OFF_CURVE,
      CURVE,
      LINE,
      OFF_CURVE,
      OFF_CURVE,
      CURVE,
      OFF_CURVE,
      OFF_CURVE,
      CURVE,
    ]);
    expect([...contourEnds]).toEqual([6, 13]);
  });

  it("rejects contour sizes that do not end on a curve point", () => {
    expect(() => makeGlyph(1, 6)).toThrow(RangeError);
    expect(() => makeGlyph(1, 1)).toThrow(RangeError);
  });
});

/** The bounding box of points `first..last` (inclusive) of a packed outline. */
function box(coords: Float64Array, first: number, last: number) {
  const xs: number[] = [];
  const ys: number[] = [];
  for (let i = first; i <= last; i++) {
    xs.push(coords[2 * i]);
    ys.push(coords[2 * i + 1]);
  }
  return {
    x0: Math.min(...xs),
    x1: Math.max(...xs),
    y0: Math.min(...ys),
    y1: Math.max(...ys),
  };
}

describe("makeNestedGlyph", () => {
  it("builds the worst-case 2 x 2,500 and 5 x 1,000 glyphs of 5,000 points", () => {
    for (const [contours, points] of [
      [2, 2500],
      [5, 1000],
    ]) {
      const glyph = makeNestedGlyph(contours, points);
      expect(glyph.flags.length).toBe(5000);
      expect(glyph.coords.length).toBe(10000);
      expect(glyph.contourEnds.length).toBe(contours);
      expect(glyph.contourEnds[contours - 1]).toBe(4999);
      expect(glyph.coords.every(Number.isFinite)).toBe(true);
    }
  });

  it("uses the same point pattern as makeGlyph", () => {
    expect(makeNestedGlyph(3, 7).flags).toEqual(makeGlyph(3, 7).flags);
  });

  it("nests the contours: each bounding box lies inside the previous one", () => {
    const { coords, contourEnds } = makeNestedGlyph(5, 1000);
    for (let c = 1; c < 5; c++) {
      const outer = box(coords, (c - 1) * 1000, c * 1000 - 1);
      const inner = box(coords, c * 1000, (c + 1) * 1000 - 1);
      expect(inner.x0).toBeGreaterThan(outer.x0);
      expect(inner.x1).toBeLessThan(outer.x1);
      expect(inner.y0).toBeGreaterThan(outer.y0);
      expect(inner.y1).toBeLessThan(outer.y1);
    }
    expect(contourEnds[4]).toBe(4999);
  });

  it("keeps points about 3 font units apart, like the grid glyph", () => {
    const { coords } = makeNestedGlyph(2, 2500);
    const gap = Math.hypot(coords[2] - coords[0], coords[3] - coords[1]);
    expect(gap).toBeGreaterThan(2);
    expect(gap).toBeLessThan(6);
  });

  it("rejects contour sizes that do not end on a curve point", () => {
    expect(() => makeNestedGlyph(2, 6)).toThrow(RangeError);
  });
});

describe("innerBox", () => {
  it("is inside the bounding box of every contour of a nested glyph", () => {
    const glyph = makeNestedGlyph(5, 1000);
    const { x0, y0, x1, y1 } = innerBox(glyph);
    for (let c = 0; c < 5; c++) {
      const b = box(glyph.coords, c * 1000, (c + 1) * 1000 - 1);
      expect(x0).toBeGreaterThanOrEqual(b.x0);
      expect(x1).toBeLessThanOrEqual(b.x1);
      expect(y0).toBeGreaterThanOrEqual(b.y0);
      expect(y1).toBeLessThanOrEqual(b.y1);
    }
    expect(x1).toBeGreaterThan(x0);
    expect(y1).toBeGreaterThan(y0);
  });
});

describe("decodeHit", () => {
  it("decodes points, segments and misses", () => {
    expect(decodeHit(new Float64Array([0, 7, 1.5]))).toEqual({
      kind: "point",
      index: 7,
      distance: 1.5,
    });
    expect(decodeHit(new Float64Array([1, 2, 4, 6, 0.25, 3]))).toEqual({
      kind: "segment",
      contour: 2,
      start: 4,
      end: 6,
      t: 0.25,
      distance: 3,
    });
    expect(decodeHit(new Float64Array())).toBeNull();
  });
});
