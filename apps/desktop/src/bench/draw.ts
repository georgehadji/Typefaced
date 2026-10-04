// How a drag frame draws the bench glyph: the `cached` design of ADR-0013 and the probe
// variants that take it apart (docs/spikes/spike-3-ipc.md, "Why the cached drag ran at 30 fps").
import { OFF_CURVE, type PackedOutline } from "./outline";

/** What one drag frame draws, and on what kind of canvas. */
export interface DrawSpec {
  /** Fill the outlines (nonzero). */
  fill: boolean;
  /** Stroke the outlines with a one-pixel line. */
  stroke: boolean;
  /** Point handles of every contour, of the dragged contours only, or none. */
  handles: "all" | "selected" | "none";
  /** Still contours redrawn as vectors every frame, or drawn once into a bitmap that each frame copies. */
  still: "vector" | "bitmap";
  /**
   * `split`: one `Path2D` for the still contours and one for the dragged ones;
   * `perContour`: one `Path2D` per contour; `combined`: one for the whole glyph, nothing moves.
   */
  paths: "split" | "perContour" | "combined";
  /** Move the dragged contours with a canvas transform; `false` draws them where they are. */
  transform: boolean;
  /** Canvas side in CSS pixels. */
  size: number;
  /** Context attributes for `getContext("2d", …)`. */
  context: CanvasRenderingContext2DSettings;
}

/** The `cached` drag mode as first measured: every handle, two cached paths, a transform. */
export const CACHED_DRAW: DrawSpec = {
  fill: true,
  stroke: true,
  handles: "all",
  still: "vector",
  paths: "split",
  transform: true,
  size: 760,
  context: {},
};

/**
 * The probes: each changes some settings of `CACHED_DRAW`. `control` changes nothing, so
 * it shows what a fresh canvas costs against the `cached` run.
 */
export const DRAG_PROBES = {
  control: {},
  fillOnly: { stroke: false, handles: "none" },
  strokeOnly: { fill: false, handles: "none" },
  outlineNoHandles: { handles: "none" },
  handlesOnly: { fill: false, stroke: false },
  selectedHandles: { handles: "selected" },
  canvas380: { size: 380 },
  desynchronized: { context: { desynchronized: true } },
  alphaFalse: { context: { alpha: false } },
  softwareCanvas: { context: { willReadFrequently: true } },
  perContourPaths: { paths: "perContour" },
  combinedPath: { paths: "combined" },
  noTransform: { transform: false },
  stillBitmap: { still: "bitmap" },
  stillBitmapSelectedHandles: { still: "bitmap", handles: "selected" },
} satisfies Record<string, Partial<DrawSpec>>;

/** The outline and, when drawn, the point handles of a run of contours. */
export interface GlyphPaths {
  outline: Path2D;
  handles?: Path2D;
}

/** Builds the paths of contours `fromContour` up to (not including) `toContour`. */
export function buildPaths(
  { coords, flags, contourEnds }: PackedOutline,
  fromContour: number,
  toContour: number,
  withHandles: boolean,
): GlyphPaths {
  const outline = new Path2D();
  const handles = withHandles ? new Path2D() : undefined;
  for (let c = fromContour; c < toContour; c++) {
    const first = c === 0 ? 0 : contourEnds[c - 1] + 1;
    const last = contourEnds[c];
    outline.moveTo(coords[2 * first], coords[2 * first + 1]);
    let i = first + 1;
    while (i <= last) {
      // The bench glyph's cubic segments are always off, off, curve.
      if (flags[i] === OFF_CURVE && i + 2 <= last) {
        outline.bezierCurveTo(
          coords[2 * i],
          coords[2 * i + 1],
          coords[2 * i + 2],
          coords[2 * i + 3],
          coords[2 * i + 4],
          coords[2 * i + 5],
        );
        i += 3;
      } else {
        outline.lineTo(coords[2 * i], coords[2 * i + 1]);
        i += 1;
      }
    }
    outline.closePath();
    for (let p = first; p <= last && handles; p++) {
      handles.rect(coords[2 * p] - 1.5, coords[2 * p + 1] - 1.5, 3, 3);
    }
  }
  return { outline, handles };
}

/** Where a frame draws: pointer offset in font units, glyph scale and canvas height in device pixels. */
export interface Frame {
  dx: number;
  dy: number;
  scale: number;
  height: number;
}

/** The cached paths of one drag: `still` drawn in place, `dragged` moved by the pointer. */
export interface Scene {
  still: GlyphPaths[];
  dragged: GlyphPaths[];
  /** The still contours drawn once, when `spec.still` is `bitmap` (then `still` is empty). */
  bitmap?: HTMLCanvasElement;
}

function runs(
  glyph: PackedOutline,
  from: number,
  to: number,
  perContour: boolean,
  withHandles: boolean,
): GlyphPaths[] {
  if (!perContour) return [buildPaths(glyph, from, to, withHandles)];
  return Array.from({ length: to - from }, (_, i) =>
    buildPaths(glyph, from + i, from + i + 1, withHandles),
  );
}

/** Sets the glyph transform: y up, font units. */
function glyphSpace(ctx: CanvasRenderingContext2D, frame: Frame) {
  ctx.setTransform(frame.scale, 0, 0, -frame.scale, 0, frame.height);
}

function setStyles(ctx: CanvasRenderingContext2D, scale: number) {
  ctx.fillStyle = "rgba(40, 40, 40, 0.25)";
  ctx.strokeStyle = "#222";
  ctx.lineWidth = 1 / scale;
}

function paint(
  ctx: CanvasRenderingContext2D,
  paths: GlyphPaths[],
  spec: DrawSpec,
) {
  for (const { outline, handles } of paths) {
    if (spec.fill) ctx.fill(outline, "nonzero");
    if (spec.stroke) ctx.stroke(outline);
    if (handles) ctx.fill(handles);
  }
}

/**
 * Builds the paths once per glyph revision. The first `draggedContours` contours are the
 * dragged selection. With `still: "bitmap"` the other contours are drawn here, once, into
 * a canvas of the frame's size.
 */
export function makeScene(
  glyph: PackedOutline,
  draggedContours: number,
  spec: DrawSpec,
  frame: Frame,
): Scene {
  const contours = glyph.contourEnds.length;
  const perContour = spec.paths === "perContour";
  if (spec.paths === "combined") {
    const all = buildPaths(glyph, 0, contours, spec.handles !== "none");
    return { still: [all], dragged: [] };
  }
  const still = runs(
    glyph,
    draggedContours,
    contours,
    perContour,
    spec.handles === "all",
  );
  const dragged = runs(
    glyph,
    0,
    draggedContours,
    perContour,
    spec.handles !== "none",
  );
  if (spec.still === "vector") return { still, dragged };
  const bitmap = document.createElement("canvas");
  // The bench canvas is square.
  bitmap.width = Math.round(frame.height);
  bitmap.height = Math.round(frame.height);
  const ctx = bitmap.getContext("2d");
  if (!ctx) throw new Error("Canvas2D is unavailable for the still bitmap");
  glyphSpace(ctx, frame);
  setStyles(ctx, frame.scale);
  paint(ctx, still, spec);
  return { still: [], dragged, bitmap };
}

/** Draws one drag frame of `scene` on `ctx`. */
export function paintFrame(
  ctx: CanvasRenderingContext2D,
  scene: Scene,
  spec: DrawSpec,
  frame: Frame,
) {
  glyphSpace(ctx, frame);
  ctx.clearRect(0, 0, 1000, 1000);
  if (scene.bitmap) {
    ctx.save();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.drawImage(scene.bitmap, 0, 0);
    ctx.restore();
  }
  setStyles(ctx, frame.scale);
  paint(ctx, scene.still, spec);
  if (spec.transform) ctx.translate(frame.dx, frame.dy);
  paint(ctx, scene.dragged, spec);
}
