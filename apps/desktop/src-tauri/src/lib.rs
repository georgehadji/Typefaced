//! Typefaced desktop driver: the Tauri shell that exposes typed commands to the UI.
//!
//! Every IPC command is registered in [`specta_builder`], the single source of truth
//! for both the running app and the generated TypeScript bindings.

mod ai;
#[cfg(feature = "bench")]
mod bench;

use tauri::Manager;
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

/// Builds the IPC surface. Used by [`run`] and by the tests (bindings export, ACL).
/// Every command here also needs an `allow-` permission: see `APP_COMMANDS` in
/// `build.rs` and `capabilities/default.json`.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        app_info,
        ai::ai_set_key,
        ai::ai_has_key,
        ai::ai_delete_key,
        ai::ai_fetch,
        ai::ai_abort,
    ])
}

/// Starts the desktop application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[allow(
    clippy::expect_used,
    reason = "the app cannot run without its window runtime; failing loudly at start-up is intended"
)]
pub fn run() {
    let typed = specta_builder().invoke_handler();
    let app = tauri::Builder::default().setup(|app| {
        // AI is optional: without a host (e.g. no OS keychain) the app still starts and
        // the `ai_*` commands answer with an error.
        match ai::host(app.handle()) {
            Ok(host) => {
                app.manage(host);
            }
            Err(error) => eprintln!("AI is unavailable: {error}"),
        }
        #[cfg(feature = "bench")]
        bench::open_bench_page(app)?;
        Ok(())
    });

    // Dev-only benchmarks: their raw commands are routed by name, outside tauri-specta.
    #[cfg(feature = "bench")]
    let (app, typed) = {
        let bench = bench::invoke_handler();
        let handler = move |invoke: tauri::ipc::Invoke| {
            if invoke.message.command().starts_with(bench::COMMAND_PREFIX) {
                bench(invoke)
            } else {
                typed(invoke)
            }
        };
        (app.manage(bench::BenchState::default()), handler)
    };

    app.invoke_handler(typed)
        .run(tauri::generate_context!())
        .expect("error while running the Typefaced application");
}

#[cfg(test)]
mod tests {
    use super::specta_builder;
    use serde_json::{Value, json};
    use specta_typescript::Typescript;
    use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponseBody};
    use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
    use tauri::webview::InvokeRequest;
    use tauri::{Manager, WebviewWindow, WebviewWindowBuilder};
    use tf_ai_host::{CredentialVault, EgressHost, EgressPolicy, UsageLedger};

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

    fn window(app: &tauri::App<MockRuntime>, label: &str) -> WebviewWindow<MockRuntime> {
        app.get_webview_window(label).unwrap_or_else(|| {
            WebviewWindowBuilder::new(app, label, Default::default())
                .build()
                .unwrap()
        })
    }

    fn invoke(
        window: &WebviewWindow<MockRuntime>,
        cmd: &str,
        body: Value,
    ) -> Result<InvokeResponseBody, Value> {
        get_ipc_response(
            window,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_owned(),
            },
        )
    }

    /// The real capabilities (`generate_context!`) grant the app commands to the `main`
    /// window only; any other window is refused before a command runs.
    #[cfg(windows)]
    #[test]
    fn only_the_main_window_may_call_the_app_commands() {
        // A test-only keychain entry that this test never writes.
        let vault = CredentialVault::os(
            "com.typefaced.desktop.test",
            &format!("acl-test-{}", std::process::id()),
        )
        .unwrap();
        let ledger = UsageLedger::new(std::env::temp_dir().join("typefaced-acl-test.jsonl"));
        let host = EgressHost::new(EgressPolicy::anthropic(), vault, ledger).unwrap();
        // `app_info` takes a Wry `AppHandle`, so the mock app registers only the `ai_*`
        // commands; the ACL check runs before dispatch, so `app_info` is still checked.
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![
                crate::ai::ai_set_key,
                crate::ai::ai_has_key,
                crate::ai::ai_delete_key,
                crate::ai::ai_fetch,
                crate::ai::ai_abort,
            ])
            .manage(host)
            .build(tauri::generate_context!())
            .unwrap();
        let main = window(&app, "main");
        let other = window(&app, "other");

        let aborted = invoke(&main, "ai_abort", json!({ "requestId": "r1" })).unwrap();
        assert!(!aborted.deserialize::<bool>().unwrap());
        // Allowed by the ACL; refused only because the mock app does not register it.
        let unregistered = invoke(&main, "app_info", json!({})).unwrap_err();
        assert!(
            !unregistered.to_string().contains("not allowed"),
            "{unregistered}"
        );

        for cmd in [
            "app_info",
            "ai_set_key",
            "ai_has_key",
            "ai_delete_key",
            "ai_fetch",
            "ai_abort",
        ] {
            let refused = invoke(&other, cmd, json!({})).unwrap_err();
            assert!(
                refused.to_string().contains("not allowed"),
                "{cmd}: {refused}"
            );
        }
    }
}
