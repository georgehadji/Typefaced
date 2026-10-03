import process from "node:process";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],

  // The WASM kernel is imported with `?inline` (a data URL), which needs .wasm to be an
  // asset: see src/bench/kernel.ts.
  assetsInclude: ["**/*.wasm"],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },

  // Vitest reads this file too, so tests always use the app's Vite settings.
  test: {
    environment: "jsdom",
    // Used by `pnpm test:ci` (`vitest run --coverage`), which CI runs.
    coverage: {
      provider: "v8",
      // Only this app's code: the generated bindings are another package.
      include: ["src/**/*.{ts,tsx}"],
      // The entry point only mounts the page. The dev-only benchmark runners need the
      // real WebView (Tauri IPC, WASM, Canvas2D, requestAnimationFrame), which jsdom
      // lacks; they are validated by running `#/bench` (docs/spikes/spike-3-ipc.md).
      exclude: [
        "src/main.tsx",
        "src/bench/runners.ts",
        "src/**/*.test.{ts,tsx}",
        "src/**/*.d.ts",
      ],
      thresholds: { lines: 80, functions: 80, branches: 80, statements: 80 },
    },
  },
});
