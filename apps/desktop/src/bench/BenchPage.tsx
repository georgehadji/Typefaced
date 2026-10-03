import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import {
  benchCommit,
  benchDrag,
  benchFrameBaseline,
  benchHitTest,
  benchPatchStream,
  benchPing,
  type HitLayout,
} from "./runners";
import "./bench.css";

/** Calls a bench command and ignores a failure: reporting must never mask the real error. */
async function sendToRust(command: string, args: Record<string, unknown>) {
  try {
    await invoke(command, args);
  } catch {
    // Nothing more can be done from the page.
  }
}

/** Dev-only page (`#/bench`): runs every M0 Step 8 benchmark once, then reports to Rust. */
export default function BenchPage() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const started = useRef(false);
  const [log, setLog] = useState<string[]>([]);

  useEffect(() => {
    // The benchmarks must run once, even if React re-runs the effect (StrictMode).
    if (started.current || !canvas.current) return;
    started.current = true;
    const target = canvas.current;
    // Progress also goes to the Rust log, so an unattended run shows where it stopped.
    const step = (line: string) => {
      setLog((lines) => [...lines, line]);
      void sendToRust("bench_log", { line });
    };
    const runAll = async () => {
      step("page loaded");
      const environment = {
        userAgent: navigator.userAgent,
        devicePixelRatio: window.devicePixelRatio,
        hardwareConcurrency: navigator.hardwareConcurrency,
        screen: { width: screen.width, height: screen.height },
        visibility: document.visibilityState,
        hasFocus: document.hasFocus(),
      };
      step(
        "empty IPC round trip (1,000 × main thread, 1,000 × async runtime)…",
      );
      const ping = await benchPing();
      step(
        "commit round-trip (1,000 × JSON, 1,000 × binary, at 1 to 5,000 points)…",
      );
      const commit = await benchCommit();
      step("patch stream, 1-point patches (60 Hz, 10 s)…");
      const streamSmall = await benchPatchStream(1);
      step("patch stream, 5,000-point patches (60 Hz, 10 s)…");
      const streamLarge = await benchPatchStream(5000);
      step(
        "hit test (1,000 calls, radius 8; then radius 1, which finds segments)…",
      );
      const hitTest = await benchHitTest();
      const hitTestSegments = await benchHitTest(1000, 1);
      step(
        "hit test, worst case: few large nested contours (2 x 2,500 and 5 x 1,000 points)…",
      );
      const hitTestWorstCase = {} as Record<
        HitLayout,
        { radius8: unknown; radius1: unknown }
      >;
      for (const layout of ["nested2x2500", "nested5x1000"] as const) {
        hitTestWorstCase[layout] = {
          radius8: await benchHitTest(1000, 8, layout),
          radius1: await benchHitTest(1000, 1, layout),
        };
      }
      step("empty animation frames (3 s)…");
      const frameBaseline = await benchFrameBaseline();
      step("drag, hit test and translate only (10 s)…");
      const compute = await benchDrag(target, 10_000, "compute");
      step("drag, glyph rebuilt every frame (10 s)…");
      const rebuild = await benchDrag(target, 10_000, "rebuild");
      step("drag, cached Path2D objects (10 s)…");
      const cached = await benchDrag(target, 10_000, "cached");
      const results = {
        environment,
        visibilityAfter: document.visibilityState,
        ping,
        commit,
        stream: { small: streamSmall, large: streamLarge },
        hitTest,
        hitTestSegments,
        hitTestWorstCase,
        frameBaseline,
        drag: { compute, rebuild, cached },
      };
      step(JSON.stringify(results, null, 2));
      await invoke("bench_report", { results });
      step("reported to Rust (target/bench-results.json)");
    };
    runAll().catch((error: unknown) => {
      step(`failed: ${String(error)}`);
      void sendToRust("bench_report", { results: { error: String(error) } });
    });
  }, []);

  return (
    <main className="bench">
      <h1>IPC and WASM benchmarks</h1>
      <canvas ref={canvas} />
      <pre aria-live="polite">{log.join("\n")}</pre>
    </main>
  );
}
