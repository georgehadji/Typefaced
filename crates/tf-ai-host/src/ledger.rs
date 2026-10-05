//! Usage ledger stub: one JSON line per forwarded request (request ID, model, status,
//! duration, byte counts) in a file in the app-data folder. Never bodies, headers or keys.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use serde::Serialize;

/// Longest model name or request ID written, in characters.
const MAX_FIELD_CHARS: usize = 128;

/// One forwarded request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRecord {
    /// The API's `request-id` response header.
    pub request_id: Option<String>,
    /// The `model` field of the JSON request body.
    pub model: Option<String>,
    /// HTTP status, if a response arrived.
    pub status: Option<u16>,
    pub duration_ms: u64,
    pub request_bytes: u64,
    pub response_bytes: u64,
}

impl UsageRecord {
    /// The `model` field of a JSON body, shortened to a safe length.
    pub fn model_of(body: Option<&str>) -> Option<String> {
        #[derive(serde::Deserialize)]
        struct Model {
            model: Option<String>,
        }
        let model = serde_json::from_str::<Model>(body?).ok()?.model?;
        Some(shorten(&model))
    }
}

/// Shortens untrusted text before it is written.
pub(crate) fn shorten(text: &str) -> String {
    text.chars().take(MAX_FIELD_CHARS).collect()
}

/// An append-only JSONL file.
#[derive(Debug)]
pub struct UsageLedger {
    path: PathBuf,
    write: Mutex<()>,
}

impl UsageLedger {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            write: Mutex::new(()),
        }
    }

    /// Appends `record` as one line, creating the file and its folder if needed.
    pub fn append(&self, record: &UsageRecord) -> io::Result<()> {
        let mut line = serde_json::to_string(record)?;
        line.push('\n');
        // One writer at a time, so concurrent requests never interleave their lines.
        let _guard = self.write.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?
            .write_all(line.as_bytes())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A ledger path in the system temp folder, unique per test and process, so tests
    /// never write into the repository.
    pub(crate) fn temp_ledger(name: &str) -> (UsageLedger, PathBuf) {
        let dir = std::env::temp_dir().join(format!("tf-ai-host-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("nested").join("ai-usage.jsonl");
        (UsageLedger::new(&path), path)
    }

    #[test]
    fn appends_one_json_line_per_record() {
        let (ledger, path) = temp_ledger("append");
        let record = UsageRecord {
            request_id: Some("req_1".into()),
            model: Some("claude-opus-5".into()),
            status: Some(200),
            duration_ms: 12,
            request_bytes: 34,
            response_bytes: 56,
        };
        ledger.append(&record).unwrap();
        ledger.append(&UsageRecord::default()).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines,
            [
                r#"{"requestId":"req_1","model":"claude-opus-5","status":200,"durationMs":12,"requestBytes":34,"responseBytes":56}"#,
                r#"{"requestId":null,"model":null,"status":null,"durationMs":0,"requestBytes":0,"responseBytes":0}"#,
            ]
        );
        std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn reads_the_model_from_a_json_body() {
        assert_eq!(
            UsageRecord::model_of(Some(r#"{"model":"claude-opus-5","messages":[]}"#)),
            Some("claude-opus-5".into())
        );
        assert_eq!(UsageRecord::model_of(Some(r#"{"messages":[]}"#)), None);
        assert_eq!(UsageRecord::model_of(Some("not json")), None);
        assert_eq!(UsageRecord::model_of(None), None);
        let long = format!(r#"{{"model":"{}"}}"#, "m".repeat(500));
        assert_eq!(
            UsageRecord::model_of(Some(&long)).unwrap().len(),
            MAX_FIELD_CHARS
        );
    }

    #[test]
    fn reports_an_unwritable_path() {
        let (ledger, path) = temp_ledger("blocked");
        // A file where the folder should be.
        std::fs::create_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
        std::fs::write(path.parent().unwrap(), b"").unwrap();
        assert!(ledger.append(&UsageRecord::default()).is_err());
        std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
    }
}
