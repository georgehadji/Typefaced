//! Typed command catalog: the API contract between the engine and every client
//! (UI, AI, MCP, CLI). Types here are exported to TypeScript through tauri-specta.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Name and version of the running application, as shown in the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AppInfo {
    /// Product name, e.g. `Typefaced`.
    pub name: String,
    /// Semantic version of the application, e.g. `0.1.0`.
    pub version: String,
}

#[cfg(test)]
mod tests {
    use super::AppInfo;
    use serde_json::json;

    #[test]
    fn app_info_serializes_to_the_ipc_json_shape() {
        let info = AppInfo {
            name: "Typefaced".to_owned(),
            version: "0.1.0".to_owned(),
        };

        let value = serde_json::to_value(&info).unwrap();

        assert_eq!(value, json!({ "name": "Typefaced", "version": "0.1.0" }));
    }

    #[test]
    fn app_info_round_trips_through_json() {
        let json = r#"{ "name": "Typefaced", "version": "2.3.4" }"#;

        let info: AppInfo = serde_json::from_str(json).unwrap();

        assert_eq!(info.name, "Typefaced");
        assert_eq!(info.version, "2.3.4");
    }
}
