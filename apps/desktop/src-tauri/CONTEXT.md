# apps/desktop/src-tauri/

Rust crate `typefaced-desktop`: the Tauri 2 shell, layer `driver`. It registers the
typed IPC commands and serves the frontend from `../dist`. Up: [apps/desktop/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: lib `desktop_lib`, depends on `tauri`, `tauri-specta`, `specta`, `tf-commands`; the TypeScript exporter is a dev-dependency only (used by the `export_bindings` test) |
| `build.rs` | Runs tauri-build. On Windows MSVC it embeds `windows-app-manifest.xml` through the linker for every binary, so `cargo test` executables start (tauri-apps/tauri#13419) |
| `windows-app-manifest.xml` | Windows manifest that selects Common Controls v6 |
| `tauri.conf.json` | Tauri config: product name, version, identifier `com.typefaced.desktop`, dev URL, 1280×800 window, strict CSP, bundle icons |
| `capabilities/default.json` | Tauri capability for the `main` window: `core:default` permissions only. (No `CONTEXT.md` in that folder: Tauri reads every file there.) |
| `.gitignore` | Ignores `target/` and the generated `gen/schemas` |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): Rust entry point and IPC registration |
| `icons/` | [CONTEXT.md](icons/CONTEXT.md): app icons |
| `capabilities/` | See `default.json` above |
