//! Reducers of the outline family: `glyph.setOutline`, `glyph.movePoints`,
//! `glyph.insertPoint`, `glyph.deletePoints`, `glyph.toggleSmooth`, `glyph.transform`.
//! Step 6 writes them; until then each fails with `E_NOT_IMPLEMENTED`.

use tf_commands::command::{
    GlyphDeletePoints, GlyphInsertPoint, GlyphMovePoints, GlyphSetOutline, GlyphToggleSmooth,
    GlyphTransform,
};

not_implemented! {
    set_outline(GlyphSetOutline) = "glyph.setOutline";
    move_points(GlyphMovePoints) = "glyph.movePoints";
    insert_point(GlyphInsertPoint) = "glyph.insertPoint";
    delete_points(GlyphDeletePoints) = "glyph.deletePoints";
    toggle_smooth(GlyphToggleSmooth) = "glyph.toggleSmooth";
    transform(GlyphTransform) = "glyph.transform";
}
