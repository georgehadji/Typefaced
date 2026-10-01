import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import {
  benchCommit,
  benchDrag,
  benchHitTest,
  benchPatchStream,
} from "./runners";
import "./bench.css";

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
    const step = (line: string) => setLog((lines) => [...lines, line]);
    const runAll = async () => {
      const environment = {
        userAgent: navigator.userAgent,
        devicePixelRatio: window.devicePixelRatio,
        hardwareConcurrency: navigator.hardwareConcurrency,
        screen: { width: screen.width, height: screen.height },
        visibility: document.visibilityState,
        hasFocus: document.hasFocus(),
      };
      step("commit round-trip (1,000 × JSON, 1,000 × binary)…");
      const commit = await benchCommit();
      step("patch stream, 1-point patches (60 Hz, 10 s)…");
      const streamSmall = await benchPatchStream(1);
      step("patch stream, 5,000-point patches (60 Hz, 10 s)…");
      const streamLarge = await benchPatchStream(5000);
      step("hit test (1,000 calls)…");
      const hitTest = await benchHitTest();
      step("drag (10 s)…");
      const drag = await benchDrag(target);
      const results = {
        environment,
        visibilityAfter: document.visibilityState,
        commit,
        stream: { small: streamSmall, large: streamLarge },
        hitTest,
        drag,
      };
      step(JSON.stringify(results, null, 2));
      await invoke("bench_report", { results });
      step("reported to Rust (target/bench-results.json)");
    };
    runAll().catch((error: unknown) => step(`failed: ${String(error)}`));
  }, []);

  return (
    <main className="bench">
      <h1>IPC and WASM benchmarks</h1>
      <canvas ref={canvas} />
      <pre aria-live="polite">{log.join("\n")}</pre>
    </main>
  );
}
