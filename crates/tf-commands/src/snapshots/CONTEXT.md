# crates/tf-commands/src/snapshots/

insta JSON snapshots of the strict JSON Schema of every command and query payload, one
file per catalog id. The test `every_payload_schema_matches_its_snapshot` in
`registry.rs` compares them: a changed snapshot is a change to the API contract, so
review it. Regenerate with `INSTA_UPDATE=always cargo test -p tf-commands`.
Up: [src/CONTEXT.md](../CONTEXT.md).

| File | Payload schema of |
|---|---|
| `doc.close.snap` | `doc.close` |
| `doc.new.snap` | `doc.new` |
| `doc.open.snap` | `doc.open` |
| `doc.redo.snap` | `doc.redo` |
| `doc.save.snap` | `doc.save` |
| `doc.saveAs.snap` | `doc.saveAs` |
| `doc.undo.snap` | `doc.undo` |
| `features.setCode.snap` | `features.setCode` |
| `font.setInfo.snap` | `font.setInfo` |
| `font.setVerticalMetrics.snap` | `font.setVerticalMetrics` |
| `glyph.addAnchor.snap` | `glyph.addAnchor` |
| `glyph.addComponent.snap` | `glyph.addComponent` |
| `glyph.create.snap` | `glyph.create` |
| `glyph.decompose.snap` | `glyph.decompose` |
| `glyph.delete.snap` | `glyph.delete` |
| `glyph.deletePoints.snap` | `glyph.deletePoints` |
| `glyph.duplicate.snap` | `glyph.duplicate` |
| `glyph.insertPoint.snap` | `glyph.insertPoint` |
| `glyph.moveAnchor.snap` | `glyph.moveAnchor` |
| `glyph.movePoints.snap` | `glyph.movePoints` |
| `glyph.removeAnchor.snap` | `glyph.removeAnchor` |
| `glyph.removeComponent.snap` | `glyph.removeComponent` |
| `glyph.rename.snap` | `glyph.rename` |
| `glyph.setCodepoints.snap` | `glyph.setCodepoints` |
| `glyph.setOutline.snap` | `glyph.setOutline` |
| `glyph.setWidth.snap` | `glyph.setWidth` |
| `glyph.toggleSmooth.snap` | `glyph.toggleSmooth` |
| `glyph.transform.snap` | `glyph.transform` |
| `kerning.removePairs.snap` | `kerning.removePairs` |
| `kerning.setGroup.snap` | `kerning.setGroup` |
| `kerning.setPairs.snap` | `kerning.setPairs` |
| `query.diff.snap` | `query.diff` |
| `query.features.snap` | `query.features` |
| `query.fontSummary.snap` | `query.fontSummary` |
| `query.getGlyph.snap` | `query.getGlyph` |
| `query.kerning.snap` | `query.kerning` |
| `query.listGlyphs.snap` | `query.listGlyphs` |
