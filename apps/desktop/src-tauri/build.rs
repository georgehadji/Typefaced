//! Runs tauri-build. On Windows (MSVC) it also embeds the application manifest
//! through the linker, so that test executables get it too.
//!
//! Tauri links against Common Controls v6 (`TaskDialogIndirect` and others).
//! tauri-build embeds the manifest that selects v6 only into the app executable,
//! so `cargo test` binaries fail to start with STATUS_ENTRYPOINT_NOT_FOUND
//! (tauri-apps/tauri#13419). Passing the manifest to the linker for every target
//! fixes that; tauri-build is told not to embed a second copy.

const MANIFEST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/windows-app-manifest.xml");

fn main() {
    let windows = if is_windows_msvc() {
        println!("cargo:rerun-if-changed={MANIFEST}");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{MANIFEST}");
        tauri_build::WindowsAttributes::new_without_app_manifest()
    } else {
        tauri_build::WindowsAttributes::new()
    };

    let attributes = tauri_build::Attributes::new().windows_attributes(windows);
    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("tauri-build failed: {error:#}");
    }
}

fn is_windows_msvc() -> bool {
    let target_var = |name| std::env::var(name).unwrap_or_default();
    target_var("CARGO_CFG_TARGET_OS") == "windows" && target_var("CARGO_CFG_TARGET_ENV") == "msvc"
}
