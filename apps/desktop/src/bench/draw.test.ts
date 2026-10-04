import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  buildPaths,
  CACHED_DRAW,
  DRAG_PROBES,
  type DrawSpec,
  makeScene,
  paintFrame,
} from "./draw";
import { makeGlyph } from "./outline";

/** Records the path commands, so tests can count contours, segments and handle squares. */
class FakePath2D {
  ops: string[] = [];
  moveTo() {
    this.ops.push("M");
  }
  lineTo() {
    this.ops.push("L");
  }
  bezierCurveTo() {
    this.ops.push("C");
  }
  closePath() {
    this.ops.push("Z");
  }
  rect() {
    this.ops.push("R");
  }
}

const count = (path: unknown, op: string) =>
  (path as FakePath2D).ops.filter((o) => o === op).length;

/** A 2D context that records every call by name. */
function fakeContext() {
  const calls: { name: string; args: unknown[] }[] = [];
  const record =
    (name: string) =>
    (...args: unknown[]) => {
      calls.push({ name, args });
    };
  const ctx = {
    setTransform: record("setTransform"),
    clearRect: record("clearRect"),
    fill: record("fill"),
    stroke: record("stroke"),
    translate: record("translate"),
    drawImage: record("drawImage"),
    save: record("save"),
    restore: record("restore"),
    fillStyle: "",
    strokeStyle: "",
    lineWidth: 1,
  };
  return { ctx: ctx as unknown as CanvasRenderingContext2D, calls };
}

const spec = (overrides: Partial<DrawSpec>): DrawSpec => ({
  ...CACHED_DRAW,
  ...overrides,
});
const glyph = makeGlyph();
const FRAME = { dx: 3, dy: -4, scale: 0.76, height: 760 };

describe("buildPaths", () => {
  beforeEach(() => {
    vi.stubGlobal("Path2D", FakePath2D);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("builds each contour as a line start, cubic segments and a close", () => {
    const { outline, handles } = buildPaths(glyph, 0, 2, true);
    // 100 points = 1 line point + 33 cubic segments of three points.
    expect(count(outline, "M")).toBe(2);
    expect(count(outline, "C")).toBe(66);
    expect(count(outline, "Z")).toBe(2);
    expect(count(handles, "R")).toBe(200);
  });

  it("leaves the handles out when they are not drawn", () => {
    expect(buildPaths(glyph, 0, 1, false).handles).toBeUndefined();
  });

  it("draws a trailing point that cannot start a cubic as a line", () => {
    // Three points: line, off, off; the off-curve points have no segment end.
    const odd = {
      coords: new Float64Array(6),
      flags: new Uint8Array([1, 0, 0]),
      contourEnds: new Uint32Array([2]),
    };
    const { outline } = buildPaths(odd, 0, 1, false);
    expect((outline as unknown as FakePath2D).ops).toEqual([
      "M",
      "L",
      "L",
      "Z",
    ]);
  });
});

describe("makeScene", () => {
  beforeEach(() => {
    vi.stubGlobal("Path2D", FakePath2D);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("splits the glyph into still and dragged runs, with every handle (cached)", () => {
    const scene = makeScene(glyph, 10, CACHED_DRAW, FRAME);
    expect(scene.still).toHaveLength(1);
    expect(scene.dragged).toHaveLength(1);
    expect(count(scene.still[0].outline, "M")).toBe(40);
    expect(count(scene.dragged[0].outline, "M")).toBe(10);
    expect(count(scene.still[0].handles, "R")).toBe(4000);
    expect(count(scene.dragged[0].handles, "R")).toBe(1000);
    expect(scene.bitmap).toBeUndefined();
  });

  it("draws handles for the dragged contours only, or none", () => {
    const selected = makeScene(glyph, 10, spec({ handles: "selected" }), FRAME);
    expect(selected.still[0].handles).toBeUndefined();
    expect(count(selected.dragged[0].handles, "R")).toBe(1000);
    const none = makeScene(glyph, 10, spec({ handles: "none" }), FRAME);
    expect(none.still[0].handles).toBeUndefined();
    expect(none.dragged[0].handles).toBeUndefined();
  });

  it("builds one path per contour", () => {
    const scene = makeScene(glyph, 10, spec({ paths: "perContour" }), FRAME);
    expect(scene.still).toHaveLength(40);
    expect(scene.dragged).toHaveLength(10);
  });

  it("builds one path for the whole glyph, which does not move", () => {
    const scene = makeScene(glyph, 10, spec({ paths: "combined" }), FRAME);
    expect(scene.dragged).toHaveLength(0);
    expect(count(scene.still[0].outline, "M")).toBe(50);
    expect(count(scene.still[0].handles, "R")).toBe(5000);
  });

  it("draws the still contours once into a bitmap of the canvas size", () => {
    const { ctx, calls } = fakeContext();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
      ctx as never,
    );
    const scene = makeScene(glyph, 10, spec({ still: "bitmap" }), FRAME);
    expect(scene.still).toHaveLength(0);
    expect(scene.bitmap).toBeInstanceOf(HTMLCanvasElement);
    expect((scene.bitmap as HTMLCanvasElement).width).toBe(760);
    // Outline fill, stroke, then the still handles, in glyph space.
    expect(calls.map((c) => c.name)).toEqual([
      "setTransform",
      "fill",
      "stroke",
      "fill",
    ]);
    expect(calls[0].args).toEqual([0.76, 0, 0, -0.76, 0, 760]);
  });

  it("fails clearly when the bitmap has no 2D context", () => {
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    expect(() =>
      makeScene(glyph, 10, spec({ still: "bitmap" }), FRAME),
    ).toThrow(/Canvas2D/);
  });
});

describe("paintFrame", () => {
  beforeEach(() => {
    vi.stubGlobal("Path2D", FakePath2D);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("draws still contours, then the dragged ones moved by a transform (cached)", () => {
    const { ctx, calls } = fakeContext();
    paintFrame(
      ctx,
      makeScene(glyph, 10, CACHED_DRAW, FRAME),
      CACHED_DRAW,
      FRAME,
    );
    expect(calls.map((c) => c.name)).toEqual([
      "setTransform",
      "clearRect",
      "fill",
      "stroke",
      "fill",
      "translate",
      "fill",
      "stroke",
      "fill",
    ]);
    expect(calls[0].args).toEqual([0.76, 0, 0, -0.76, 0, 760]);
    expect(calls[2].args[1]).toBe("nonzero");
    expect(calls[5].args).toEqual([3, -4]);
  });

  it("skips what the spec turns off", () => {
    const off = spec({ fill: false, stroke: false, transform: false });
    const { ctx, calls } = fakeContext();
    paintFrame(ctx, makeScene(glyph, 10, off, FRAME), off, FRAME);
    // Only the two handle fills are left, and nothing moves.
    expect(calls.map((c) => c.name)).toEqual([
      "setTransform",
      "clearRect",
      "fill",
      "fill",
    ]);
  });

  it("blits the still bitmap in device pixels before the dragged contours", () => {
    const bitmap = fakeContext();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
      bitmap.ctx as never,
    );
    const still = spec({ still: "bitmap", handles: "selected" });
    const scene = makeScene(glyph, 10, still, FRAME);
    const { ctx, calls } = fakeContext();
    paintFrame(ctx, scene, still, FRAME);
    expect(calls.map((c) => c.name)).toEqual([
      "setTransform",
      "clearRect",
      "save",
      "setTransform",
      "drawImage",
      "restore",
      "translate",
      "fill",
      "stroke",
      "fill",
    ]);
    expect(calls[3].args).toEqual([1, 0, 0, 1, 0, 0]);
    expect(calls[4].args).toEqual([scene.bitmap, 0, 0]);
  });
});

describe("DRAG_PROBES", () => {
  it("names each probe once and changes at least one setting from cached", () => {
    const names = Object.keys(DRAG_PROBES);
    expect(new Set(names).size).toBe(names.length);
    expect(DRAG_PROBES.control).toEqual({});
    for (const [name, overrides] of Object.entries(DRAG_PROBES)) {
      if (name === "control") continue;
      expect(Object.keys(overrides).length, name).toBeGreaterThan(0);
    }
  });
});
