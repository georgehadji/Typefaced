//! Opaque, ordered plist values: everything the model carries but no M1 command reads.

use std::sync::Arc;

use indexmap::IndexMap;

/// A plist dictionary. Keeps the source's key order; never sorted by the model.
pub type LibDict = IndexMap<String, LibValue>;

/// The lib key norad manages itself: it moves object libs onto their objects when it loads a
/// UFO and refuses to save a font or glyph lib that still holds it. Font and glyph libs in the
/// model never contain it; layer libs may, because norad treats it as an ordinary key there.
pub const OBJECT_LIBS_KEY: &str = "public.objectLibs";

/// One XML plist value.
#[derive(Clone, Debug, PartialEq)]
pub enum LibValue {
    Dict(LibDict),
    Array(Vec<LibValue>),
    String(String),
    /// Covers both plist integer ranges (`i64` and `u64`).
    Integer(i128),
    Real(f64),
    Boolean(bool),
    /// The date as the plist's XML text (ISO 8601, e.g. `2024-01-31T12:00:00Z`).
    Date(String),
    Data(Arc<[u8]>),
}

impl LibValue {
    /// The value as a number, for `Integer` and `Real`.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            // Font values fit in f64's 53-bit mantissa; precision loss is harmless here.
            Self::Integer(value) => Some(*value as f64),
            Self::Real(value) => Some(*value),
            _ => None,
        }
    }

    /// The value as text, for `String`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_strings_read_back() {
        assert_eq!(LibValue::Integer(-3).as_f64(), Some(-3.0));
        assert_eq!(LibValue::Real(0.5).as_f64(), Some(0.5));
        assert_eq!(LibValue::Boolean(true).as_f64(), None);
        assert_eq!(LibValue::String("x".into()).as_str(), Some("x"));
        assert_eq!(LibValue::Integer(1).as_str(), None);
    }

    #[test]
    fn dictionaries_keep_insertion_order() {
        let mut dict = LibDict::new();
        dict.insert("z".into(), LibValue::Boolean(false));
        dict.insert("a".into(), LibValue::Data(Arc::from(&b"\x00\x01"[..])));
        dict.insert("m".into(), LibValue::Date("2024-01-31T12:00:00Z".into()));
        let keys: Vec<&str> = dict.keys().map(String::as_str).collect();
        assert_eq!(keys, ["z", "a", "m"]);
        let nested = LibValue::Array(vec![LibValue::Dict(dict.clone())]);
        assert_eq!(nested.clone(), nested);
    }
}
