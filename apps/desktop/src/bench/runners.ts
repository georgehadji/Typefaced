// The M0 Step 8 measurements. They need the real WebView: Tauri IPC, the WASM kernel,
// Canvas2D and requestAnimationFrame, none of which jsdom provides, so this file is
// excluded from unit-test coverage and validated by running `#/bench` (docs/spikes/spike-3-ipc.md).
import { Channel, invoke } from "@tauri-apps/api/core";
import { hitTest, translatePoints } from "@typefaced/geometry-wasm";
import {
  buildPaths,
  CACHED_DRAW,
  type DrawSpec,
  makeScene,
  paintFrame,
  type Scene,
} from "./draw";
import { loadKernel } from "./kernel";
import {
  decodeHit,
  innerBox,
  makeGlyph,
  makeNestedGlyph,
  type PackedOutline,
} from "./outline";
import { type Summary, summarize } from "./stats";

/** One frame at 60 Hz, the budget for a drag frame and the lateness threshold for patches. */
const FRAME_MS = 1000 / 60;
/** Hit radius in font units (a few screen pixels at typical zoom). */
const HIT_RADIUS = 8;
/** Binary packets start with a 16-byte header (see `src-tauri/src/bench.rs`). */
const HEADER_BYTES = 16;

/** Points per commit edit: a few moved points up to the whole 5,000-point glyph. */
export const COMMIT_SIZES = [1, 100, 1000, 5000] as const;

export interface CommitSizeResult {
  /** Points in the edit (and in the patch that comes back). */
  points: number;
  payloadBytes: { json: number; binary: number };
  json: Summary;
  binary: Summary;
}

export interface CommitResult {
  iterations: number;
  bySize: CommitSizeResult[];
}

export interface StreamResult {
  points: number;
  sent: number;
  received: number;
  latency: Summary;
  /** Patches delivered more than one 60 Hz frame after they were sent. */
  late: number;
  /** Gap between consecutive deliveries. */
  interArrival: Summary;
}

/** What a drag frame draws, see `benchDrag`. */
export type DragMode = "compute" | "rebuild" | "cached";

export interface DragResult {
  mode: DragMode;
  durationMs: number;
  frames: number;
  selectedPoints: number;
  /** requestAnimationFrame timestamp deltas. */
  frameTime: Summary;
  /** Script time per frame: hit test + translate + Path2D build + draw calls. */
  workTime: Summary;
  /** Frames whose delta exceeded 1.5 frames, i.e. at least one missed vsync. */
  longFrames: number;
  /** The drawing settings, for a probe run (see `DRAG_PROBES` in `draw.ts`). */
  draw?: DrawSpec;
}

/**
 * What the hit test runs on, all 5,000 points:
 * - `grid`: 50 contours of 100 points, bounding boxes apart (the best case: the hit test
 *   skips about 49 of 50 contours by their box);
 * - `nested2x2500` and `nested5x1000`: few large concentric contours, a pointer inside
 *   every box (the worst case: nothing can be skipped by contour).
 */
export type HitLayout = "grid" | "nested2x2500" | "nested5x1000";

export interface HitTestResult {
  layout: HitLayout;
  calls: number;
  /** Hit radius in font units. */
  radius: number;
  perCall: Summary;
  /** Total time of all calls divided by their number (finer than the clock's resolution). */
  meanFromBatchMs: number;
  hits: { point: number; segment: number; none: number };
}

/** The current time in milliseconds since the Unix epoch, with sub-millisecond precision. */
function epochNow(): number {
  return performance.timeOrigin + performance.now();
}

/** Moves every point of the outline by (dx, dy): the "whole glyph edited" payload. */
function shifted(glyph: PackedOutline, dx: number): Float64Array {
  return glyph.coords.map((v, i) => (i % 2 === 0 ? v + dx : v));
}

type Patch = { revision: number; coords: number[] };

export async function benchCommit(
  iterations = 1000,
  warmup = 20,
): Promise<CommitResult> {
  const glyph = makeGlyph();
  const run = async (once: (i: number) => Promise<number>) => {
    for (let i = 0; i < warmup; i++) await once(i);
    const samples: number[] = [];
    for (let i = 0; i < iterations; i++) samples.push(await once(i));
    return summarize(samples);
  };
  const bySize: CommitSizeResult[] = [];
  for (const points of COMMIT_SIZES) {
    const part: PackedOutline = {
      ...glyph,
      coords: glyph.coords.slice(0, points * 2),
    };
    const timeJson = async (i: number) => {
      const coords = shifted(part, i % 10);
      const t0 = performance.now();
      const patch = await invoke<Patch>("bench_commit_json", {
        edit: { coords: Array.from(coords) },
      });
      const replica = Float64Array.from(patch.coords);
      const elapsed = performance.now() - t0;
      if (replica.length !== coords.length)
        throw new Error("JSON patch has the wrong size");
      return elapsed;
    };
    const timeBinary = async (i: number) => {
      const coords = shifted(part, i % 10);
      const t0 = performance.now();
      const buffer = await invoke<ArrayBuffer>(
        "bench_commit_binary",
        new Uint8Array(coords.buffer),
      );
      const replica = new Float64Array(buffer, HEADER_BYTES);
      const elapsed = performance.now() - t0;
      if (replica.length !== coords.length)
        throw new Error("binary patch has the wrong size");
      return elapsed;
    };
    const json = await run(timeJson);
    const binary = await run(timeBinary);
    bySize.push({
      points,
      payloadBytes: {
        json: JSON.stringify({ edit: { coords: Array.from(part.coords) } })
          .length,
        binary: part.coords.byteLength,
      },
      json,
      binary,
    });
  }
  return { iterations, bySize };
}

export interface PingResult {
  /** `invoke` of a command that returns nothing, handled on the main thread. */
  main: Summary;
  /** The same, handled on the async runtime (as the commit commands are). */
  asyncRuntime: Summary;
}

/** The floor of one IPC round trip: no payload, no work. */
export async function benchPing(
  iterations = 1000,
  warmup = 20,
): Promise<PingResult> {
  const run = async (command: string) => {
    for (let i = 0; i < warmup; i++) await invoke(command);
    const samples: number[] = [];
    for (let i = 0; i < iterations; i++) {
      const t0 = performance.now();
      await invoke(command);
      samples.push(performance.now() - t0);
    }
    return summarize(samples);
  };
  return {
    main: await run("bench_ping"),
    asyncRuntime: await run("bench_ping_async"),
  };
}

export async function benchPatchStream(
  points: number,
  count = 600,
  intervalMs = FRAME_MS,
): Promise<StreamResult> {
  const latencies: number[] = [];
  const arrivals: number[] = [];
  const done = new Promise<void>((resolve) => {
    const channel = new Channel<ArrayBuffer>((message) => {
      const arrived = epochNow();
      const view = new DataView(message);
      const sentAt = view.getFloat64(8, true);
      const patch = new Float64Array(message, HEADER_BYTES);
      if (patch.length !== points * 2)
        throw new Error("patch has the wrong size");
      latencies.push(arrived - sentAt);
      arrivals.push(arrived);
      if (arrivals.length === count) resolve();
    });
    void invoke("bench_patch_stream", {
      onPatch: channel,
      count,
      points,
      intervalMs,
    });
  });
  const timeout = new Promise<void>((resolve) =>
    setTimeout(resolve, count * intervalMs + 5000),
  );
  await Promise.race([done, timeout]);
  const gaps = arrivals.slice(1).map((t, i) => t - arrivals[i]);
  return {
    points,
    sent: count,
    received: latencies.length,
    latency: summarize(latencies),
    late: latencies.filter((ms) => ms > FRAME_MS).length,
    interArrival: summarize(gaps),
  };
}

function hitGlyph(layout: HitLayout): PackedOutline {
  switch (layout) {
    case "grid":
      return makeGlyph();
    case "nested2x2500":
      return makeNestedGlyph(2, 2500);
    case "nested5x1000":
      return makeNestedGlyph(5, 1000);
  }
}

export async function benchHitTest(
  calls = 1000,
  radius = HIT_RADIUS,
  layout: HitLayout = "grid",
): Promise<HitTestResult> {
  await loadKernel();
  const glyph = hitGlyph(layout);
  // Deterministic positions: over the grid glyph's bounding box, or inside the innermost
  // ring of a nested glyph (inside the box of every contour).
  const { x0, y0, x1, y1 } =
    layout === "grid" ? { x0: 0, y0: 0, x1: 900, y1: 800 } : innerBox(glyph);
  let seed = 1;
  const random = () => {
    seed = (seed * 16807) % 2147483647;
    return seed / 2147483647;
  };
  const positions = Array.from({ length: calls }, () => [
    x0 + random() * (x1 - x0),
    y0 + random() * (y1 - y0),
  ]);
  const perCall: number[] = [];
  const hits = { point: 0, segment: 0, none: 0 };
  const start = performance.now();
  for (const [x, y] of positions) {
    const t0 = performance.now();
    const hit = decodeHit(
      hitTest(glyph.coords, glyph.flags, glyph.contourEnds, x, y, radius),
    );
    perCall.push(performance.now() - t0);
    hits[hit?.kind ?? "none"]++;
  }
  return {
    layout,
    calls,
    radius,
    perCall: summarize(perCall),
    meanFromBatchMs: (performance.now() - start) / calls,
    hits,
  };
}

/** Empty animation frames: the cadence this window can reach before any work is done. */
export function benchFrameBaseline(durationMs = 3000): Promise<Summary> {
  return new Promise((resolve) => {
    const deltas: number[] = [];
    let startedAt: number | undefined;
    let previous: number | undefined;
    const frame = (now: number) => {
      startedAt ??= now;
      if (previous !== undefined) deltas.push(now - previous);
      previous = now;
      if (now - startedAt < durationMs) requestAnimationFrame(frame);
      else resolve(summarize(deltas));
    };
    requestAnimationFrame(frame);
  });
}

/** Contours in the drag selection: ten contours, 1,000 points, a large selection. */
const DRAGGED_CONTOURS = 10;

/**
 * Puts a fresh canvas in place of `canvas` for a probe, so its context attributes apply
 * (a canvas keeps the attributes of its first `getContext`). Returns the fresh canvas and
 * a function that puts the original back.
 */
function probeCanvas(canvas: HTMLCanvasElement, size: number) {
  const fresh = canvas.cloneNode() as HTMLCanvasElement;
  fresh.style.width = `${size}px`;
  fresh.style.height = `${size}px`;
  canvas.replaceWith(fresh);
  return { fresh, restore: () => fresh.replaceWith(canvas) };
}

/**
 * A synthetic drag: every animation frame moves the pointer along a circle, hit-tests
 * under it and translates the selected contours with the WASM kernel, then draws:
 * - `compute`: nothing (the cadence the kernel alone allows);
 * - `rebuild`: the whole moved glyph, with new `Path2D` objects built every frame;
 * - `cached`: `Path2D` objects built once per glyph revision (ADR-0013); the dragged
 *   contours are drawn with a canvas transform and the rest are not rebuilt.
 *
 * `probe` (with `cached`) changes how the frame is drawn, on a fresh canvas: see
 * `DRAG_PROBES` in `draw.ts`. The result then carries the `draw` settings used.
 */
export async function benchDrag(
  canvas: HTMLCanvasElement,
  durationMs = 10_000,
  mode: DragMode = "rebuild",
  probe?: Partial<DrawSpec>,
): Promise<DragResult> {
  await loadKernel();
  const glyph = makeGlyph();
  const draggedPoints = glyph.contourEnds[DRAGGED_CONTOURS - 1] + 1;
  const selection = Uint32Array.from({ length: draggedPoints }, (_, i) => i);
  const spec: DrawSpec = { ...CACHED_DRAW, ...probe };
  const swap = probe ? probeCanvas(canvas, spec.size) : undefined;
  const target = swap?.fresh ?? canvas;
  const dpr = window.devicePixelRatio || 1;
  const size = target.clientWidth;
  target.width = Math.round(size * dpr);
  target.height = Math.round(size * dpr);
  const base = {
    dx: 0,
    dy: 0,
    scale: (size * dpr) / 1000,
    height: target.height,
  };
  let ctx: CanvasRenderingContext2D;
  let scene: Scene | undefined;
  try {
    const context = target.getContext("2d", spec.context);
    if (!context) throw new Error("Canvas2D is unavailable");
    ctx = context;
    scene =
      mode === "cached"
        ? makeScene(glyph, DRAGGED_CONTOURS, spec, base)
        : undefined;
  } catch (error) {
    swap?.restore(); // A failed probe must not leave the page without its canvas.
    throw error;
  }
  const deltas: number[] = [];
  const work: number[] = [];
  return new Promise((resolve) => {
    let startedAt: number | undefined;
    let previous: number | undefined;
    const frame = (now: number) => {
      startedAt ??= now;
      if (previous !== undefined) deltas.push(now - previous);
      previous = now;
      const t0 = performance.now();
      const phase = ((now - startedAt) / 2000) * 2 * Math.PI;
      const dx = 40 * Math.cos(phase);
      const dy = 40 * Math.sin(phase);
      hitTest(
        glyph.coords,
        glyph.flags,
        glyph.contourEnds,
        500 + dx,
        400 + dy,
        HIT_RADIUS,
      );
      const moved = translatePoints(glyph.coords, selection, dx, dy);
      if (scene) {
        paintFrame(ctx, scene, spec, { ...base, dx, dy });
      } else if (mode === "rebuild") {
        const all = { ...glyph, coords: moved };
        const paths = buildPaths(all, 0, glyph.contourEnds.length, true);
        // Nothing is dragged separately, so no transform (as before the probes).
        paintFrame(
          ctx,
          { still: [paths], dragged: [] },
          { ...spec, transform: false },
          base,
        );
      }
      work.push(performance.now() - t0);
      if (now - startedAt < durationMs) {
        requestAnimationFrame(frame);
        return;
      }
      swap?.restore();
      resolve({
        mode,
        durationMs: now - startedAt,
        frames: deltas.length,
        selectedPoints: selection.length,
        frameTime: summarize(deltas),
        workTime: summarize(work),
        longFrames: deltas.filter((ms) => ms > 1.5 * FRAME_MS).length,
        ...(probe ? { draw: spec } : {}),
      });
    };
    requestAnimationFrame(frame);
  });
}
