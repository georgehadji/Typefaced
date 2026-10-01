import { describe, expect, it } from "vitest";
import { CURVE, decodeHit, LINE, makeGlyph, OFF_CURVE } from "./outline";

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
