# apps/desktop/

The desktop app package `@typefaced/desktop`: the React UI (`src/`) and the Tauri shell
(`src-tauri/`). Build the frontend before any cargo command: the Tauri crate embeds
`dist/`. Up: [apps/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `package.json` | Package manifest: React 19, the workspace bindings, Vite, Vitest, Testing Library, Tauri CLI. Scripts: `dev`, `build` (`tsc && vite build`), `tauri`, `typecheck`, `test`, `test:ci` (with coverage) |
| `index.html` | HTML entry page; mounts `#root` and loads `src/main.tsx` |
| `vite.config.ts` | Vite config for Tauri (fixed port 1420, ignores `src-tauri`) and the Vitest config (jsdom, v8 coverage of `src/` with 80% thresholds) |
| `tsconfig.json` | TypeScript config for the browser code in `src/` (strict, no Node types) |
| `tsconfig.node.json` | TypeScript config for `vite.config.ts` (Node types) |
| `.gitignore` | Ignores logs, `node_modules`, `dist` and editor files |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): React UI |
| `src-tauri/` | [CONTEXT.md](src-tauri/CONTEXT.md): Tauri shell crate |
| `dist/` | Build output (not tracked) |
