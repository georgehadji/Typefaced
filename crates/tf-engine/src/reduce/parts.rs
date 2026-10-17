//! Reducers of the components-and-anchors family: `glyph.addComponent`,
//! `glyph.removeComponent`, `glyph.decompose`, `glyph.addAnchor`, `glyph.moveAnchor`,
//! `glyph.removeAnchor`. Step 6 writes them; until then each fails with `E_NOT_IMPLEMENTED`.

use tf_commands::command::{
    GlyphAddAnchor, GlyphAddComponent, GlyphDecompose, GlyphMoveAnchor, GlyphRemoveAnchor,
    GlyphRemoveComponent,
};

/// The commands of this family that have a reducer (checked by the dispatch test).
#[cfg(test)]
pub(super) const IMPLEMENTED: &[tf_commands::CommandId] = &[];

not_implemented! {
    add_component(GlyphAddComponent) = "glyph.addComponent";
    remove_component(GlyphRemoveComponent) = "glyph.removeComponent";
    decompose(GlyphDecompose) = "glyph.decompose";
    add_anchor(GlyphAddAnchor) = "glyph.addAnchor";
    move_anchor(GlyphMoveAnchor) = "glyph.moveAnchor";
    remove_anchor(GlyphRemoveAnchor) = "glyph.removeAnchor";
}
