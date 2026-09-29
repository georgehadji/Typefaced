//! Typefaced desktop driver: the Tauri shell that exposes typed commands to the UI.
//!
//! Every IPC command is registered in [`specta_builder`], the single source of truth
//! for both the running app and the generated TypeScript bindings.

use tauri_specta::{Builder, collect_commands};
use tf_commands::AppInfo;

/// Returns the product name and version configured in `tauri.conf.json`.
#[tauri::command]
#[specta::specta]
fn app_info(app: tauri::AppHandle) -> AppInfo {
    let package = app.package_info();
    AppInfo {
        name: package.name.clone(),
        version: package.version.to_string(),
    }
}

/// Builds the IPC surface. Used by [`run`] and by the `export_bindings` test.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![app_info])
}

/// Starts the desktop application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[allow(
    clippy::expect_used,
    reason = "the app cannot run without its window runtime; failing loudly at start-up is intended"
)]
pub fn run() {
    let builder = specta_builder();
    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running the Typefaced application");
}

#[cfg(test)]
mod tests {
    use super::specta_builder;
    use specta_typescript::Typescript;

    /// Regenerates `packages/bindings/src/index.ts`. CI fails when the committed
    /// file differs from the output, so the bindings can never drift from Rust.
    #[test]
    fn export_bindings() {
        specta_builder()
            .export(
                Typescript::default(),
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../packages/bindings/src/index.ts"
                ),
            )
            .expect("failed to export the TypeScript bindings");
    }
}
