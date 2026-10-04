/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** `drag` runs only the frame and drag benchmarks on `#/bench` (see src/bench/BenchPage.tsx). */
  readonly VITE_BENCH_ONLY?: string;
}
