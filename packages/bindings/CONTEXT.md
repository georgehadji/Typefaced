# packages/bindings/

Package `@typefaced/bindings`: typed TypeScript wrappers for the Tauri IPC commands,
generated from Rust by tauri-specta. The package exports `src/index.ts` directly (no
build step). Up: [packages/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `package.json` | Package manifest: exports `./src/index.ts`; depends on `@tauri-apps/api`; `typecheck` script |
| `tsconfig.json` | Strict TypeScript config for `src/` |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): the generated bindings |
