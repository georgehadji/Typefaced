import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Node has fetch's Response, ReadableStream and crypto.randomUUID; the Tauri IPC is mocked.
    environment: "node",
    coverage: {
      provider: "v8",
      include: ["src/**/*.ts"],
      exclude: ["src/**/*.test.ts", "src/fakeProxy.ts"],
      thresholds: { lines: 80, functions: 80, branches: 80, statements: 80 },
    },
  },
});
