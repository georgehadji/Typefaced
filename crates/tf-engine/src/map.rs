//! DTO ↔ model conversions (plan deviation 4: commands carry wire DTOs, the engine maps
//! them to `tf-model` types) and the mapping of model errors to stable error codes.

use tf_commands::command as dto;
use tf_commands::{CommandError, ErrorCode};
use tf_model::{GlyphId, LayerId, ModelError, Name};

pub(crate) fn glyph_id(id: dto::GlyphId) -> GlyphId {
    GlyphId(id.0)
}

pub(crate) fn layer_id(id: dto::LayerId) -> LayerId {
    LayerId(id.0)
}

/// A glyph name; `E_INVALID_NAME` when it breaks the UFO name rules.
pub(crate) fn name(name: &str) -> Result<Name, CommandError> {
    Name::new(name).map_err(model_error)
}

/// A Unicode scalar value; `E_INVALID_FIELD` for a surrogate or a value above U+10FFFF.
pub(crate) fn codepoint(value: u32) -> Result<char, CommandError> {
    char::from_u32(value).ok_or_else(|| {
        CommandError::new(
            ErrorCode::InvalidField,
            format!("U+{value:04X} is not a Unicode scalar value"),
        )
    })
}

/// A finite number of font units; `E_INVALID_FIELD` for NaN or an infinity (a NaN would
/// also make two equal fonts compare unequal).
pub(crate) fn units(field: &str, value: f64) -> Result<f64, CommandError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(CommandError::new(
            ErrorCode::InvalidField,
            format!("{field} must be a finite number, not {value}"),
        ))
    }
}

/// The stable code of a model error; the message is the model's.
pub(crate) fn model_error(error: ModelError) -> CommandError {
    use ModelError as M;
    // No wildcard: a new model error must pick its code here.
    let code = match &error {
        M::InvalidName(_) => ErrorCode::InvalidName,
        M::NameInUse(_) | M::DuplicateLayerName { .. } => ErrorCode::NameInUse,
        M::GlyphNotFound(_) => ErrorCode::GlyphNotFound,
        M::SourceNotFound(_) => ErrorCode::SourceNotFound,
        M::LayerNotFound(_) => ErrorCode::LayerNotFound,
        M::InvalidContour { .. } => ErrorCode::InvalidOutline,
        M::TableFull => ErrorCode::LimitExceeded,
        M::InvalidIdentifier(_)
        | M::InvalidColor(_)
        | M::LayerInUse(_)
        | M::DuplicateLayerId(_)
        | M::NoLayers(_)
        | M::SourceExists(_)
        | M::DuplicateCodepoint { .. }
        | M::DuplicateIdentifier { .. }
        | M::ObjectLibWithoutIdentifier { .. }
        | M::NoGlyphLayers(_)
        | M::ReservedKey(_) => ErrorCode::InvalidField,
    };
    CommandError::new(code, error.to_string())
}

#[cfg(test)]
mod tests {
    use tf_model::{ContourError, Identifier, ObjectOwner, SourceId};

    use super::*;

    fn n(s: &str) -> Name {
        Name::new(s).unwrap()
    }

    #[test]
    fn ids_and_values_map_to_the_model() {
        assert_eq!(glyph_id(dto::GlyphId(3)), GlyphId(3));
        assert_eq!(layer_id(dto::LayerId(2)), LayerId(2));
        assert_eq!(name("A.alt").unwrap().as_str(), "A.alt");
        assert_eq!(name("").unwrap_err().code, ErrorCode::InvalidName);
        assert_eq!(codepoint(0x41), Ok('A'));
        let surrogate = codepoint(0xD800).unwrap_err();
        assert_eq!(surrogate.code, ErrorCode::InvalidField);
        assert!(surrogate.message.contains("U+D800"));
        assert_eq!(
            codepoint(0x11_0000).unwrap_err().code,
            ErrorCode::InvalidField
        );
        assert_eq!(units("width", -5.5), Ok(-5.5));
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                units("width", bad).unwrap_err().code,
                ErrorCode::InvalidField
            );
        }
    }

    #[test]
    fn every_model_error_has_a_code() {
        let owner = ObjectOwner::FontInfo(SourceId(0));
        let cases = [
            (M::InvalidName("".into()), ErrorCode::InvalidName),
            (M::NameInUse(n("A")), ErrorCode::NameInUse),
            (
                M::DuplicateLayerName {
                    source_id: SourceId(0),
                    name: n("l"),
                },
                ErrorCode::NameInUse,
            ),
            (M::GlyphNotFound(GlyphId(1)), ErrorCode::GlyphNotFound),
            (M::SourceNotFound(SourceId(1)), ErrorCode::SourceNotFound),
            (M::LayerNotFound(LayerId(1)), ErrorCode::LayerNotFound),
            (
                M::InvalidContour {
                    glyph: n("A"),
                    layer: LayerId(0),
                    contour: 0,
                    reason: ContourError::Empty,
                },
                ErrorCode::InvalidOutline,
            ),
            (M::TableFull, ErrorCode::LimitExceeded),
            (M::InvalidIdentifier("x".into()), ErrorCode::InvalidField),
            (M::InvalidColor(2.0), ErrorCode::InvalidField),
            (M::LayerInUse(LayerId(0)), ErrorCode::InvalidField),
            (M::DuplicateLayerId(LayerId(0)), ErrorCode::InvalidField),
            (M::NoLayers(SourceId(0)), ErrorCode::InvalidField),
            (M::SourceExists(SourceId(0)), ErrorCode::InvalidField),
            (
                M::DuplicateCodepoint {
                    glyph: n("A"),
                    layer: LayerId(0),
                    codepoint: 'A',
                },
                ErrorCode::InvalidField,
            ),
            (
                M::DuplicateIdentifier {
                    owner: owner.clone(),
                    identifier: Identifier::new("i").unwrap(),
                },
                ErrorCode::InvalidField,
            ),
            (
                M::ObjectLibWithoutIdentifier { owner },
                ErrorCode::InvalidField,
            ),
            (M::NoGlyphLayers(n("A")), ErrorCode::InvalidField),
            (M::ReservedKey("k".into()), ErrorCode::InvalidField),
        ];
        for (error, code) in cases {
            let message = error.to_string();
            let mapped = model_error(error);
            assert_eq!((mapped.code, mapped.message), (code, message));
        }
    }

    use ModelError as M;
}
