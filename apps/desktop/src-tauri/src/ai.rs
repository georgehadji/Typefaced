//! AI egress commands (ADR-0009): store, check and delete the Claude API key, forward a
//! request through `tf-ai-host` and stream the response over a `Channel`, and abort it.
//! No command returns the key: `tf-ai-host` gives this crate no way to read it.

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Runtime, State};
use tf_ai_host::{
    CredentialVault, EgressHost, EgressPolicy, ProxyEvent, ProxyRequest, UsageLedger,
};

/// The usage ledger's file name in the app-data folder.
const LEDGER_FILE: &str = "ai-usage.jsonl";

/// The egress host for the app: production policy and keychain entry, ledger in app data.
pub fn host<R: Runtime>(app: &AppHandle<R>) -> Result<EgressHost, Box<dyn std::error::Error>> {
    let ledger = UsageLedger::new(app.path().app_data_dir()?.join(LEDGER_FILE));
    Ok(EgressHost::new(
        EgressPolicy::anthropic(),
        CredentialVault::production()?,
        ledger,
    )?)
}

/// Stores the Claude API key in the OS keychain, replacing any earlier key.
#[tauri::command(async)]
#[specta::specta]
pub fn ai_set_key(host: State<'_, EgressHost>, key: String) -> Result<(), String> {
    host.vault().set_key(&key).map_err(|e| e.to_string())
}

/// Whether a Claude API key is stored.
#[tauri::command(async)]
#[specta::specta]
pub fn ai_has_key(host: State<'_, EgressHost>) -> Result<bool, String> {
    host.vault().has_key().map_err(|e| e.to_string())
}

/// Deletes the stored Claude API key, if any.
#[tauri::command(async)]
#[specta::specta]
pub fn ai_delete_key(host: State<'_, EgressHost>) -> Result<(), String> {
    host.vault().delete_key().map_err(|e| e.to_string())
}

/// Forwards one request to the Claude API with the stored key. `on_event` receives the
/// response head first, then the body chunks, then an end, error or aborted marker. The
/// command itself resolves when the response is over.
#[tauri::command]
#[specta::specta]
pub async fn ai_fetch(
    host: State<'_, EgressHost>,
    request: ProxyRequest,
    on_event: Channel<ProxyEvent>,
) -> Result<(), String> {
    host.fetch(request, |event| on_event.send(event).is_ok())
        .await;
    Ok(())
}

/// Cancels the in-flight request `request_id`. Returns whether there was one.
#[tauri::command]
#[specta::specta]
pub fn ai_abort(host: State<'_, EgressHost>, request_id: String) -> bool {
    host.abort(&request_id)
}
