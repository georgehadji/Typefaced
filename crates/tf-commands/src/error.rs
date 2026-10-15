//! `CommandError` and every stable error code of M1 (implementation plan §10.1). A code
//! serializes as its string, e.g. `"E_GLYPH_NOT_FOUND"`, so the UI, the AI agent and MCP
//! clients can react to it. Later steps use these codes; a new code is added here.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specta::Type;

macro_rules! error_codes {
    ($( $(#[$meta:meta])* $variant:ident = $code:literal, )*) => {
        /// A stable, machine-readable error code.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type, JsonSchema)]
        pub enum ErrorCode {
            $( $(#[$meta])* #[serde(rename = $code)] $variant, )*
        }

        impl ErrorCode {
            /// Every code, in declaration order.
            pub const ALL: &[ErrorCode] = &[$(ErrorCode::$variant),*];

            /// The code as it appears on the wire, e.g. `E_GLYPH_NOT_FOUND`.
            pub fn as_str(self) -> &'static str {
                match self { $(ErrorCode::$variant => $code,)* }
            }
        }
    };
}

error_codes! {
    /// No glyph has the given id or name.
    GlyphNotFound = "E_GLYPH_NOT_FOUND",
    /// No layer has the given id.
    LayerNotFound = "E_LAYER_NOT_FOUND",
    /// No source has the given id.
    SourceNotFound = "E_SOURCE_NOT_FOUND",
    /// Another glyph (or group) already has this name.
    NameInUse = "E_NAME_IN_USE",
    /// The name is empty or not allowed.
    InvalidName = "E_INVALID_NAME",
    /// Another glyph already has this code point.
    CodepointInUse = "E_CODEPOINT_IN_USE",
    /// Other glyphs use this glyph as a component.
    GlyphInUse = "E_GLYPH_IN_USE",
    /// The outline, a point index or a packed payload is invalid.
    InvalidOutline = "E_INVALID_OUTLINE",
    /// A field key or value is not accepted.
    InvalidField = "E_INVALID_FIELD",
    /// Nested components go deeper than the limit.
    ComponentDepth = "E_COMPONENT_DEPTH",
    /// Components refer to each other in a cycle.
    ComponentCycle = "E_COMPONENT_CYCLE",
    /// The command was made against an older revision.
    StaleBase = "E_STALE_BASE",
    /// No transaction is open.
    NoTransaction = "E_NO_TRANSACTION",
    /// A transaction is already open.
    TransactionOpen = "E_TRANSACTION_OPEN",
    /// No sandbox has the given id.
    SandboxNotFound = "E_SANDBOX_NOT_FOUND",
    /// The undo stack is empty.
    NothingToUndo = "E_NOTHING_TO_UNDO",
    /// The redo stack is empty.
    NothingToRedo = "E_NOTHING_TO_REDO",
    /// The command exists in the catalog but has no implementation yet.
    NotImplemented = "E_NOT_IMPLEMENTED",
    /// The document is read-only.
    ReadOnly = "E_READ_ONLY",
    /// The target path exists and `overwrite` is not set.
    TargetExists = "E_TARGET_EXISTS",
    /// The document has unsaved changes and `discard` is not set.
    UnsavedChanges = "E_UNSAVED_CHANGES",
    /// The files on disk changed since they were loaded.
    ExternalChange = "E_EXTERNAL_CHANGE",
    /// An input exceeds a size or count limit.
    LimitExceeded = "E_LIMIT_EXCEEDED",
    /// A path points outside the project.
    PathOutsideProject = "E_PATH_OUTSIDE_PROJECT",
    /// A `.glif` file holds XML that norad cannot read (e.g. comments in `<outline>`).
    UfoUnsupportedXml = "E_UFO_UNSUPPORTED_XML",
    /// The `.typefaced` package is malformed.
    PackageInvalid = "E_PACKAGE_INVALID",
    /// Saving failed; the files on disk are unchanged.
    SaveFailed = "E_SAVE_FAILED",
    /// Another input/output error.
    Io = "E_IO",
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The error every command and query returns: a stable code and a message for humans.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema, thiserror::Error,
)]
#[error("{code}: {message}")]
pub struct CommandError {
    /// The stable code.
    pub code: ErrorCode,
    /// What went wrong, in plain words.
    pub message: String,
}

impl CommandError {
    /// An error with the given code and message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashSet;

    #[test]
    fn every_m1_code_exists_once() {
        let codes: HashSet<&str> = ErrorCode::ALL.iter().map(|c| c.as_str()).collect();

        assert_eq!(ErrorCode::ALL.len(), 28);
        assert_eq!(codes.len(), ErrorCode::ALL.len());
        assert!(codes.iter().all(|c| c.starts_with("E_")));
    }

    #[test]
    fn a_code_serializes_as_its_string_and_back() {
        for &code in ErrorCode::ALL {
            let value = serde_json::to_value(code).unwrap();

            assert_eq!(value, json!(code.as_str()));
            assert_eq!(serde_json::from_value::<ErrorCode>(value).unwrap(), code);
        }
    }

    #[test]
    fn an_error_has_a_json_shape_and_a_message() {
        let error = CommandError::new(ErrorCode::GlyphNotFound, "no glyph 7");

        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            json!({ "code": "E_GLYPH_NOT_FOUND", "message": "no glyph 7" })
        );
        assert_eq!(error.to_string(), "E_GLYPH_NOT_FOUND: no glyph 7");
    }
}
