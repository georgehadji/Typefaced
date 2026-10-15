//! Stable entity IDs: `u32` newtypes, because IPC integers are 32 bits or smaller (ADR-0004).

use std::fmt;

macro_rules! id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u32);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}#{}", stringify!($name), self.0)
            }
        }
    };
}

id!(
    /// A glyph's slot in the [`GlyphTable`](crate::GlyphTable). Independent of the glyph name;
    /// never reused while the document is open (deleted glyphs leave a tombstone).
    GlyphId
);
id!(
    /// One UFO of the project (a designspace may hold several).
    SourceId
);
id!(
    /// One UFO layer of one source. Unique across the whole font.
    LayerId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_compare_by_value_and_display_their_kind() {
        assert_eq!(GlyphId(3), GlyphId(3));
        assert!(LayerId(1) < LayerId(2));
        assert_eq!(SourceId(7).to_string(), "SourceId#7");
    }
}
