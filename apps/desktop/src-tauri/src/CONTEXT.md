# apps/desktop/src-tauri/src/

Rust source of the Tauri shell. Up: [src-tauri/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `main.rs` | Binary entry point: calls `desktop_lib::run()`; hides the console window in release builds |
| `lib.rs` | `app_info` command (name and version from `tauri.conf.json`); `specta_builder()`, the single list of IPC commands used by the app and by the bindings export; `run()`, which starts Tauri, manages the AI egress host (the app still starts without it; the `ai_*` commands then fail) and, with the `bench` feature, routes `bench_` commands to `bench.rs`. Test `export_bindings` regenerates `packages/bindings/src/index.ts` (CI fails if the committed file differs); test `only_the_main_window_may_call_the_app_commands` runs the real capabilities on Tauri's mock runtime |
| `ai.rs` | AI egress commands over `tf-ai-host`: `ai_set_key`, `ai_has_key`, `ai_delete_key`, `ai_fetch` (streams `ProxyEvent`s over a `Channel`: head, chunks, then end, error or aborted) and `ai_abort`. No command returns the key. `host()` builds the egress host with the production keychain entry and the usage ledger `ai-usage.jsonl` in the app-data folder |
| `bench.rs` | Dev-only IPC benchmarks, compiled only with the `bench` feature: raw Tauri commands (outside tauri-specta) for the JSON and binary commit round trip, empty ping commands (the floor of one `invoke`), a patch stream over a `Channel`, `bench_log` (progress lines on stdout) and `bench_report`, which writes the results under `target/`; it also opens `#/bench` from the `devUrl`. The UI side is `apps/desktop/src/bench/` |

To add an IPC command: define its types in `crates/tf-commands`, add the
`#[tauri::command] #[specta::specta]` function here, list it in `collect_commands!`,
add its name to `APP_COMMANDS` in `../build.rs` and its `allow-<command>` permission
(underscores become hyphens) to `../capabilities/default.json`, then run
`cargo test -p typefaced-desktop export_bindings` and commit the bindings.
