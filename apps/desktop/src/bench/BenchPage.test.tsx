import { invoke } from "@tauri-apps/api/core";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import BenchPage from "./BenchPage";
import * as runners from "./runners";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("./runners", () => ({
  benchCommit: vi.fn(),
  benchPatchStream: vi.fn(),
  benchHitTest: vi.fn(),
  benchDrag: vi.fn(),
}));

const report = vi.mocked(invoke);

describe("BenchPage", () => {
  afterEach(() => {
    cleanup();
    vi.resetAllMocks();
  });

  it("runs every benchmark once and reports the results to Rust", async () => {
    vi.mocked(runners.benchCommit).mockResolvedValue({ commit: 1 } as never);
    vi.mocked(runners.benchPatchStream).mockImplementation(
      async (points) => ({ points }) as never,
    );
    vi.mocked(runners.benchHitTest).mockResolvedValue({ hit: 1 } as never);
    vi.mocked(runners.benchDrag).mockResolvedValue({ drag: 1 } as never);
    report.mockResolvedValue(undefined);

    render(<BenchPage />);

    expect(await screen.findByText(/reported to Rust/)).toBeTruthy();
    expect(runners.benchCommit).toHaveBeenCalledOnce();
    expect(runners.benchPatchStream).toHaveBeenCalledWith(1);
    expect(runners.benchPatchStream).toHaveBeenCalledWith(5000);
    expect(runners.benchDrag).toHaveBeenCalledWith(
      expect.any(HTMLCanvasElement),
    );
    expect(report).toHaveBeenCalledWith("bench_report", {
      results: expect.objectContaining({
        commit: { commit: 1 },
        stream: { small: { points: 1 }, large: { points: 5000 } },
        hitTest: { hit: 1 },
        drag: { drag: 1 },
      }),
    });
  });

  it("shows the failure when a benchmark rejects", async () => {
    vi.mocked(runners.benchCommit).mockRejectedValue(new Error("IPC down"));

    render(<BenchPage />);

    expect(await screen.findByText(/failed: Error: IPC down/)).toBeTruthy();
    expect(report).not.toHaveBeenCalled();
  });
});
