/// <reference types="vitest/config" />
import process from "node:process";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react()],

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
    // On in CI (GitHub sets CI=true); run locally with `pnpm test -- --coverage`.
    coverage: {
      enabled: process.env.CI === "true",
      provider: "v8" as const,
      // Only this app's code: the generated bindings are another package.
      include: ["src/**/*.{ts,tsx}"],
      // The entry point only mounts <App /> into the page.
      exclude: ["src/main.tsx", "src/**/*.test.{ts,tsx}"],
      thresholds: { lines: 80, functions: 80, branches: 80, statements: 80 },
    },
  },
}));
