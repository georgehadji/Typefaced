//! Validated names and identifiers, with the same rules as norad 0.18.4, so converting the
//! model to norad never fails on a name.

use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use crate::ModelError;

/// A UFO name: glyph, group, layer, anchor, point or guideline name. Non-empty, with no
/// control characters (C0, DEL, C1).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(Arc<str>);

impl Name {
    /// Checks `name` against the UFO name rules.
    pub fn new(name: &str) -> Result<Self, ModelError> {
        let control = |c: char| matches!(u32::from(c), 0x00..=0x1f | 0x7f..=0x9f);
        if name.is_empty() || name.chars().any(control) {
            return Err(ModelError::InvalidName(name.to_owned()));
        }
        Ok(Self(name.into()))
    }

    /// The name as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A UFO object identifier: at most 100 characters of printable ASCII (0x20–0x7E).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identifier(Arc<str>);

impl Identifier {
    /// Checks `identifier` against the UFO identifier rules.
    pub fn new(identifier: &str) -> Result<Self, ModelError> {
        if identifier.len() > 100 || !identifier.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
            return Err(ModelError::InvalidIdentifier(identifier.to_owned()));
        }
        Ok(Self(identifier.into()))
    }

    /// The identifier as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

macro_rules! str_like {
    ($name:ident) => {
        impl Deref for $name {
            type Target = str;
            fn deref(&self) -> &str {
                &self.0
            }
        }

        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

str_like!(Name);
str_like!(Identifier);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_norad_rules() {
        for good in ["A", "a.alt", "uni00C1", "space", "public.kern1.O", "ä"] {
            assert_eq!(Name::new(good).unwrap().as_str(), good);
        }
        for bad in ["", "a\tb", "a\u{7f}", "a\u{85}b", "\n"] {
            assert_eq!(Name::new(bad), Err(ModelError::InvalidName(bad.to_owned())));
        }
    }

    #[test]
    fn identifiers_are_short_printable_ascii() {
        assert!(Identifier::new("").is_ok());
        assert!(Identifier::new("abc DEF~").is_ok());
        assert!(Identifier::new(&"x".repeat(100)).is_ok());
        assert!(Identifier::new(&"x".repeat(101)).is_err());
        assert!(Identifier::new("é").is_err());
        assert!(Identifier::new("a\tb").is_err());
    }

    #[test]
    fn names_borrow_as_str_and_display_verbatim() {
        let name = Name::new("A").unwrap();
        let id = Identifier::new("id1").unwrap();
        assert_eq!(Borrow::<str>::borrow(&name), "A");
        assert_eq!(&*name, "A");
        assert_eq!(name.to_string(), "A");
        assert_eq!(id.to_string(), "id1");
        assert_eq!(&*id, "id1");
        assert_eq!(id.as_str(), "id1");
    }
}
