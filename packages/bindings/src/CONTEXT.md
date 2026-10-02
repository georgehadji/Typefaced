# packages/bindings/src/

Up: [bindings/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `index.ts` | **Generated, never edit by hand.** `commands` (one typed function per IPC command, e.g. `appInfo()`) and the payload types (e.g. `AppInfo`). Regenerate with `cargo test -p typefaced-desktop export_bindings` and commit; CI fails on drift. Biome skips this file |
