# apps/desktop/src-tauri/src/

Rust source of the Tauri shell. Up: [src-tauri/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `main.rs` | Binary entry point: calls `desktop_lib::run()`; hides the console window in release builds |
| `lib.rs` | `app_info` command (name and version from `tauri.conf.json`); `specta_builder()`, the single list of IPC commands used by the app and by the bindings export; `run()`, which starts Tauri. Test `export_bindings` regenerates `packages/bindings/src/index.ts` (CI fails if the committed file differs) |

To add an IPC command: define its types in `crates/tf-commands`, add the
`#[tauri::command] #[specta::specta]` function here, list it in `collect_commands!`,
then run `cargo test -p typefaced-desktop export_bindings` and commit the bindings.
