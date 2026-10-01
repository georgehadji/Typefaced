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

/**
 * A synthetic glyph of `contours` closed contours, each a wavy ring of one line point
 * followed by cubic segments (off, off, curve). `pointsPerContour` must be 1 + 3k.
 * The defaults give the 5,000-point glyph of the M0 Step 8 budgets.
 */
export function makeGlyph(
  contours = 50,
  pointsPerContour = 100,
): PackedOutline {
  if (pointsPerContour < 4 || (pointsPerContour - 1) % 3 !== 0) {
    throw new RangeError("pointsPerContour must be 1 + 3k with k >= 1");
  }
  const total = contours * pointsPerContour;
  const coords = new Float64Array(total * 2);
  const flags = new Uint8Array(total);
  const contourEnds = new Uint32Array(contours);
  const columns = Math.ceil(Math.sqrt(contours));
  for (let c = 0; c < contours; c++) {
    const cx = 60 + (c % columns) * 110;
    const cy = 60 + Math.floor(c / columns) * 110;
    for (let i = 0; i < pointsPerContour; i++) {
      const index = c * pointsPerContour + i;
      const angle = (2 * Math.PI * i) / pointsPerContour;
      const isOn = i % 3 === 0;
      const radius = (isOn ? 45 : 50) + 3 * Math.sin(7 * angle);
      coords[2 * index] = cx + radius * Math.cos(angle);
      coords[2 * index + 1] = cy + radius * Math.sin(angle);
      flags[index] = i === 0 ? LINE : isOn ? CURVE : OFF_CURVE;
    }
    contourEnds[c] = (c + 1) * pointsPerContour - 1;
  }
  return { coords, flags, contourEnds };
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
