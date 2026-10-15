//! Typed command catalog: the API contract between the engine and every client
//! (UI, AI, MCP, CLI). Types and metadata only, no logic (ADR-0004, implementation plan
//! §5.11). `AppInfo` is exported to TypeScript through tauri-specta; the catalog types
//! derive `specta::Type` too, but nothing registers them with tauri-specta in M1.
//!
//! - [`command`]: the mutations ([`Command`]) and their payloads;
//! - [`query`]: the reads ([`Query`]), their payloads and responses;
//! - [`registry`]: one [`Descriptor`] per command and query, and their JSON Schemas;
//! - [`error`]: [`CommandError`] and every stable error code;
//! - [`packed`]: the binary codecs of packed outlines and of `glyph.movePoints`.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Declares a catalog enum (`Command` or `Query`) and its id enum from one list, so the
/// two can never disagree. Each entry is `Variant(Payload) = "wire.id"`.
macro_rules! catalog {
    (
        $(#[$meta:meta])*
        $enum:ident, $id_enum:ident {
            $( $(#[$vmeta:meta])* $variant:ident($payload:ty) = $id:literal, )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type, schemars::JsonSchema)]
        #[serde(tag = "id", content = "args")]
        pub enum $enum {
            $( $(#[$vmeta])* #[serde(rename = $id)] $variant($payload), )*
        }

        #[doc = concat!("The id of each [`", stringify!($enum), "`] variant, without its payload.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $id_enum {
            $( $(#[$vmeta])* $variant, )*
        }

        impl $id_enum {
            /// Every id, in catalog order.
            pub const ALL: &[$id_enum] = &[$($id_enum::$variant),*];

            /// The wire id, e.g. `glyph.create`.
            pub fn as_str(self) -> &'static str {
                match self { $($id_enum::$variant => $id,)* }
            }

            /// The strict JSON Schema of this entry's payload (`args`).
            pub fn schema(self) -> schemars::Schema {
                match self { $($id_enum::$variant => $crate::registry::strict_schema::<$payload>(),)* }
            }
        }

        impl $enum {
            /// The id of this entry.
            pub fn id(&self) -> $id_enum {
                match self { $($enum::$variant(_) => $id_enum::$variant,)* }
            }
        }
    };
}

pub mod command;
pub mod error;
pub mod packed;
pub mod query;
pub mod registry;

pub use command::{Command, CommandId};
pub use error::{CommandError, ErrorCode};
pub use packed::{PackedError, PackedOutline};
pub use query::{Query, QueryId};
pub use registry::{Descriptor, Kind, Workspace};

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
