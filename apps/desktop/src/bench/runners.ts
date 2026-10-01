// The M0 Step 8 measurements. They need the real WebView: Tauri IPC, the WASM kernel,
// Canvas2D and requestAnimationFrame, none of which jsdom provides, so this file is
// excluded from unit-test coverage and validated by running `#/bench` (docs/spikes/spike-3-ipc.md).
import { Channel, invoke } from "@tauri-apps/api/core";
import initKernel, { hitTest, translatePoints } from "@typefaced/geometry-wasm";
import { decodeHit, makeGlyph, OFF_CURVE, type PackedOutline } from "./outline";
import { type Summary, summarize } from "./stats";

/** One frame at 60 Hz, the budget for a drag frame and the lateness threshold for patches. */
const FRAME_MS = 1000 / 60;
/** Hit radius in font units (a few screen pixels at typical zoom). */
const HIT_RADIUS = 8;
/** Binary packets start with a 16-byte header (see `src-tauri/src/bench.rs`). */
const HEADER_BYTES = 16;

export interface CommitResult {
  iterations: number;
  payloadBytes: { json: number; binary: number };
  json: Summary;
  binary: Summary;
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

export interface DragResult {
  durationMs: number;
  frames: number;
  selectedPoints: number;
  /** requestAnimationFrame timestamp deltas. */
  frameTime: Summary;
  /** Script time per frame: hit test + translate + Path2D build + draw calls. */
  workTime: Summary;
  /** Frames whose delta exceeded 1.5 frames, i.e. at least one missed vsync. */
  longFrames: number;
}

export interface HitTestResult {
  calls: number;
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
  const timeJson = async (i: number) => {
    const coords = shifted(glyph, i % 10);
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
    const coords = shifted(glyph, i % 10);
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
  const run = async (once: (i: number) => Promise<number>) => {
    for (let i = 0; i < warmup; i++) await once(i);
    const samples: number[] = [];
    for (let i = 0; i < iterations; i++) samples.push(await once(i));
    return summarize(samples);
  };
  const json = await run(timeJson);
  const binary = await run(timeBinary);
  return {
    iterations,
    payloadBytes: {
      json: JSON.stringify({ edit: { coords: Array.from(glyph.coords) } })
        .length,
      binary: glyph.coords.byteLength,
    },
    json,
    binary,
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

let kernelReady: Promise<unknown> | undefined;

/** Loads the WASM kernel once. */
export function loadKernel(): Promise<unknown> {
  kernelReady ??= initKernel();
  return kernelReady;
}

export async function benchHitTest(calls = 1000): Promise<HitTestResult> {
  await loadKernel();
  const glyph = makeGlyph();
  // Deterministic positions spread over the glyph's bounding box.
  let seed = 1;
  const random = () => {
    seed = (seed * 16807) % 2147483647;
    return seed / 2147483647;
  };
  const positions = Array.from({ length: calls }, () => [
    random() * 900,
    random() * 800,
  ]);
  const perCall: number[] = [];
  const hits = { point: 0, segment: 0, none: 0 };
  const start = performance.now();
  for (const [x, y] of positions) {
    const t0 = performance.now();
    const hit = decodeHit(
      hitTest(glyph.coords, glyph.flags, glyph.contourEnds, x, y, HIT_RADIUS),
    );
    perCall.push(performance.now() - t0);
    hits[hit?.kind ?? "none"]++;
  }
  return {
    calls,
    perCall: summarize(perCall),
    meanFromBatchMs: (performance.now() - start) / calls,
    hits,
  };
}

function drawGlyph(
  ctx: CanvasRenderingContext2D,
  coords: Float64Array,
  flags: Uint8Array,
  contourEnds: Uint32Array,
) {
  const outline = new Path2D();
  const handles = new Path2D();
  let first = 0;
  for (const last of contourEnds) {
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
    first = last + 1;
  }
  for (let i = 0; i < flags.length; i++) {
    handles.rect(coords[2 * i] - 1.5, coords[2 * i + 1] - 1.5, 3, 3);
  }
  ctx.fill(outline, "nonzero");
  ctx.stroke(outline);
  ctx.fill(handles);
}

/**
 * A synthetic drag: every animation frame moves the pointer along a circle, hit-tests
 * under it, translates the selected contours and redraws the whole glyph with Path2D.
 */
export async function benchDrag(
  canvas: HTMLCanvasElement,
  durationMs = 10_000,
): Promise<DragResult> {
  await loadKernel();
  const glyph = makeGlyph();
  // The drag moves ten contours (1,000 points): a large selection.
  const selection = Uint32Array.from({ length: 1000 }, (_, i) => i);
  const dpr = window.devicePixelRatio || 1;
  const size = canvas.clientWidth;
  canvas.width = Math.round(size * dpr);
  canvas.height = Math.round(size * dpr);
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Canvas2D is unavailable");
  const scale = (size * dpr) / 1000;
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
      ctx.setTransform(scale, 0, 0, -scale, 0, canvas.height);
      ctx.clearRect(0, 0, 1000, 1000);
      ctx.fillStyle = "rgba(40, 40, 40, 0.25)";
      ctx.strokeStyle = "#222";
      ctx.lineWidth = 1 / scale;
      drawGlyph(ctx, moved, glyph.flags, glyph.contourEnds);
      work.push(performance.now() - t0);
      if (now - startedAt < durationMs) {
        requestAnimationFrame(frame);
      } else {
        resolve({
          durationMs: now - startedAt,
          frames: deltas.length,
          selectedPoints: selection.length,
          frameTime: summarize(deltas),
          workTime: summarize(work),
          longFrames: deltas.filter((ms) => ms > 1.5 * FRAME_MS).length,
        });
      }
    };
    requestAnimationFrame(frame);
  });
}
