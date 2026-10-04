# apps/desktop/src-tauri/src/

Rust source of the Tauri shell. Up: [src-tauri/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `main.rs` | Binary entry point: calls `desktop_lib::run()`; hides the console window in release builds |
| `lib.rs` | `app_info` command (name and version from `tauri.conf.json`); `specta_builder()`, the single list of IPC commands used by the app and by the bindings export; `run()`, which starts Tauri and, with the `bench` feature, routes `bench_` commands to `bench.rs`. Test `export_bindings` regenerates `packages/bindings/src/index.ts` (CI fails if the committed file differs) |
| `bench.rs` | Dev-only IPC benchmarks, compiled only with the `bench` feature: raw Tauri commands (outside tauri-specta) for the JSON and binary commit round trip, empty ping commands (the floor of one `invoke`), a patch stream over a `Channel`, `bench_log` (progress lines on stdout) and `bench_report`, which writes the results under `target/`; it also opens `#/bench` from the `devUrl`. The UI side is `apps/desktop/src/bench/` |

To add an IPC command: define its types in `crates/tf-commands`, add the
`#[tauri::command] #[specta::specta]` function here, list it in `collect_commands!`,
then run `cargo test -p typefaced-desktop export_bindings` and commit the bindings.
