import { invoke } from "@tauri-apps/api/core";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import BenchPage from "./BenchPage";
import { DRAG_PROBES } from "./draw";
import * as runners from "./runners";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("./runners", () => ({
  benchCommit: vi.fn(),
  benchPatchStream: vi.fn(),
  benchHitTest: vi.fn(),
  benchPing: vi.fn(),
  benchFrameBaseline: vi.fn(),
  benchDrag: vi.fn(),
}));

const report = vi.mocked(invoke);

describe("BenchPage", () => {
  afterEach(() => {
    cleanup();
    vi.resetAllMocks();
    vi.unstubAllEnvs();
  });

  it("runs every benchmark once and reports the results to Rust", async () => {
    vi.mocked(runners.benchPing).mockResolvedValue({ ping: 1 } as never);
    vi.mocked(runners.benchCommit).mockResolvedValue({ commit: 1 } as never);
    vi.mocked(runners.benchPatchStream).mockImplementation(
      async (points) => ({ points }) as never,
    );
    vi.mocked(runners.benchHitTest).mockResolvedValue({ hit: 1 } as never);
    vi.mocked(runners.benchFrameBaseline).mockResolvedValue({
      raf: 1,
    } as never);
    vi.mocked(runners.benchDrag).mockImplementation(
      async (_canvas, _ms, mode) => ({ mode }) as never,
    );
    report.mockResolvedValue(undefined);

    render(<BenchPage />);

    expect(await screen.findByText(/reported to Rust/)).toBeTruthy();
    expect(runners.benchCommit).toHaveBeenCalledOnce();
    expect(runners.benchPatchStream).toHaveBeenCalledWith(1);
    expect(runners.benchPatchStream).toHaveBeenCalledWith(5000);
    expect(runners.benchHitTest).toHaveBeenCalledWith(1000, 1);
    // The worst case for the hit test: few large nested contours, at both radii.
    for (const layout of ["nested2x2500", "nested5x1000"]) {
      expect(runners.benchHitTest).toHaveBeenCalledWith(1000, 8, layout);
      expect(runners.benchHitTest).toHaveBeenCalledWith(1000, 1, layout);
    }
    for (const mode of ["compute", "rebuild", "cached"]) {
      expect(runners.benchDrag).toHaveBeenCalledWith(
        expect.any(HTMLCanvasElement),
        10_000,
        mode,
      );
    }
    // Every drawing probe runs once, for 5 s, on top of the cached drag.
    for (const probe of Object.values(DRAG_PROBES)) {
      expect(runners.benchDrag).toHaveBeenCalledWith(
        expect.any(HTMLCanvasElement),
        5_000,
        "cached",
        probe,
      );
    }
    expect(report).toHaveBeenCalledWith("bench_report", {
      results: expect.objectContaining({
        ping: { ping: 1 },
        commit: { commit: 1 },
        stream: { small: { points: 1 }, large: { points: 5000 } },
        hitTest: { hit: 1 },
        hitTestSegments: { hit: 1 },
        hitTestWorstCase: {
          nested2x2500: { radius8: { hit: 1 }, radius1: { hit: 1 } },
          nested5x1000: { radius8: { hit: 1 }, radius1: { hit: 1 } },
        },
        frameBaseline: { raf: 1 },
        drag: {
          compute: { mode: "compute" },
          rebuild: { mode: "rebuild" },
          cached: { mode: "cached" },
        },
        dragProbes: expect.objectContaining({
          control: { mode: "cached" },
          stillBitmapSelectedHandles: { mode: "cached" },
        }),
      }),
    });
  });

  it("runs only the frame and drag benchmarks when VITE_BENCH_ONLY is drag", async () => {
    vi.stubEnv("VITE_BENCH_ONLY", "drag");
    vi.mocked(runners.benchFrameBaseline).mockResolvedValue({
      raf: 1,
    } as never);
    vi.mocked(runners.benchDrag).mockImplementation(
      async (_canvas, _ms, mode) => ({ mode }) as never,
    );
    report.mockResolvedValue(undefined);

    render(<BenchPage />);

    expect(await screen.findByText(/reported to Rust/)).toBeTruthy();
    expect(runners.benchPing).not.toHaveBeenCalled();
    expect(runners.benchCommit).not.toHaveBeenCalled();
    expect(runners.benchHitTest).not.toHaveBeenCalled();
    const results = report.mock.calls.find(([c]) => c === "bench_report")?.[1];
    expect(results).toEqual({
      results: expect.not.objectContaining({ ping: expect.anything() }),
    });
    expect(results).toEqual({
      results: expect.objectContaining({
        frameBaseline: { raf: 1 },
        drag: expect.any(Object),
        dragProbes: expect.any(Object),
      }),
    });
  });

  it("shows the failure when a benchmark rejects", async () => {
    vi.mocked(runners.benchCommit).mockRejectedValue(new Error("IPC down"));

    render(<BenchPage />);

    expect(await screen.findByText(/failed: Error: IPC down/)).toBeTruthy();
    // An unattended run still ends: the failure goes to Rust instead of the results.
    expect(report).toHaveBeenCalledWith("bench_report", {
      results: { error: "Error: IPC down" },
    });
  });

  it("sends each progress line to the Rust log", async () => {
    vi.mocked(runners.benchCommit).mockRejectedValue(new Error("stop"));

    render(<BenchPage />);

    await screen.findByText(/failed:/);
    expect(report).toHaveBeenCalledWith("bench_log", {
      line: expect.stringContaining("commit round-trip"),
    });
  });
});
