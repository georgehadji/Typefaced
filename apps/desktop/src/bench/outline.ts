/** A packed outline (implementation plan §7.2), the layout tf-geometry works on. */
export interface PackedOutline {
  /** Interleaved `[x0, y0, x1, y1, …]` in font units. */
  coords: Float64Array;
  /** Point types, see `tf_geometry::flags`. */
  flags: Uint8Array;
  /** Index of the last point of each contour. */
  contourEnds: Uint32Array;
}

/** Point flags, mirroring `tf_geometry::flags`. */
export const OFF_CURVE = 0;
export const LINE = 1;
export const CURVE = 2;

function checkContourSize(pointsPerContour: number) {
  if (pointsPerContour < 4 || (pointsPerContour - 1) % 3 !== 0) {
    throw new RangeError("pointsPerContour must be 1 + 3k with k >= 1");
  }
}

/**
 * Builds a glyph of `contours` wavy rings of `pointsPerContour` points each: one line
 * point, then cubic segments (off, off, curve). `centre` and `radius` place contour `c`.
 */
function buildRings(
  contours: number,
  pointsPerContour: number,
  place: (c: number) => { cx: number; cy: number; radius: number },
): PackedOutline {
  checkContourSize(pointsPerContour);
  const total = contours * pointsPerContour;
  const coords = new Float64Array(total * 2);
  const flags = new Uint8Array(total);
  const contourEnds = new Uint32Array(contours);
  for (let c = 0; c < contours; c++) {
    const { cx, cy, radius: base } = place(c);
    for (let i = 0; i < pointsPerContour; i++) {
      const index = c * pointsPerContour + i;
      const angle = (2 * Math.PI * i) / pointsPerContour;
      const isOn = i % 3 === 0;
      const radius = base + (isOn ? -2.5 : 2.5) + 3 * Math.sin(7 * angle);
      coords[2 * index] = cx + radius * Math.cos(angle);
      coords[2 * index + 1] = cy + radius * Math.sin(angle);
      flags[index] = i === 0 ? LINE : isOn ? CURVE : OFF_CURVE;
    }
    contourEnds[c] = (c + 1) * pointsPerContour - 1;
  }
  return { coords, flags, contourEnds };
}

/**
 * A synthetic glyph of `contours` closed contours laid out on a grid, each a wavy ring of
 * one line point followed by cubic segments (off, off, curve). `pointsPerContour` must
 * be 1 + 3k. The defaults give the 5,000-point glyph of the M0 Step 8 budgets. This is
 * the best case for the hit test: the contours' bounding boxes are disjoint, so a
 * pointer is inside at most one of them.
 */
export function makeGlyph(
  contours = 50,
  pointsPerContour = 100,
): PackedOutline {
  const columns = Math.ceil(Math.sqrt(contours));
  return buildRings(contours, pointsPerContour, (c) => ({
    cx: 60 + (c % columns) * 110,
    cy: 60 + Math.floor(c / columns) * 110,
    radius: 47.5,
  }));
}

/**
 * The worst case for the hit test: `contours` concentric rings (like the counters of "8" or
 * a target), so the bounding box of every contour contains a pointer in the middle and
 * none of them can be skipped. The outermost ring has about 3 font units between points,
 * as in `makeGlyph`; the radii then shrink in equal steps.
 */
export function makeNestedGlyph(
  contours: number,
  pointsPerContour: number,
): PackedOutline {
  checkContourSize(pointsPerContour);
  const outer = pointsPerContour / 2;
  return buildRings(contours, pointsPerContour, (c) => ({
    cx: outer + 60,
    cy: outer + 60,
    radius: (outer * (contours - c)) / contours,
  }));
}

/** An axis-aligned box in font units. */
export interface Box {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/**
 * The intersection of the bounding boxes of all contours. For a nested glyph it is the box
 * of the innermost ring: a pointer inside it is inside every contour's box. It is empty
 * (`x1 < x0`) for a glyph whose contours are apart.
 */
export function innerBox({ coords, contourEnds }: PackedOutline): Box {
  const result: Box = {
    x0: -Infinity,
    y0: -Infinity,
    x1: Infinity,
    y1: Infinity,
  };
  let first = 0;
  for (const last of contourEnds) {
    let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
    for (let i = first; i <= last; i++) {
      x0 = Math.min(x0, coords[2 * i]);
      x1 = Math.max(x1, coords[2 * i]);
      y0 = Math.min(y0, coords[2 * i + 1]);
      y1 = Math.max(y1, coords[2 * i + 1]);
    }
    result.x0 = Math.max(result.x0, x0);
    result.y0 = Math.max(result.y0, y0);
    result.x1 = Math.min(result.x1, x1);
    result.y1 = Math.min(result.y1, y1);
    first = last + 1;
  }
  return result;
}

/** A decoded `hitTest` result from the WASM kernel. */
export type Hit =
  | { kind: "point"; index: number; distance: number }
  | {
      kind: "segment";
      contour: number;
      start: number;
      end: number;
      t: number;
      distance: number;
    };

/** Decodes the flat array returned by `hitTest` (see `crates/tf-wasm`). */
export function decodeHit(raw: Float64Array): Hit | null {
  switch (raw[0]) {
    case 0:
      return { kind: "point", index: raw[1], distance: raw[2] };
    case 1:
      return {
        kind: "segment",
        contour: raw[1],
        start: raw[2],
        end: raw[3],
        t: raw[4],
        distance: raw[5],
      };
    default:
      return null;
  }
}
