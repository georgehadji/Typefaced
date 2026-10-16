# crates/tf-commands/src/

Up: [tf-commands/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `lib.rs` | Crate root: the `catalog!` macro that declares a catalog enum and its id enum from one list; `AppInfo { name, version }`, the type exported to TypeScript today |
| `command.rs` | The 31 commands (`Command`, `CommandId`) and their payload DTOs: `u32` id newtypes, transforms, kerning pairs, `fontinfo` values. `Command::kind` makes `doc.saveAs` with `overwrite` destructive |
| `query.rs` | The 6 queries (`Query`, `QueryId`), their payloads and their response DTOs (`FontSummary`, `GlyphPage`, `GlyphDetail`, `Kerning`, `Features`) |
| `registry.rs` | One `Descriptor` per command and query (title, description, example, kind, workspaces, AI exposure, parallel safety); the `fontinfo.plist` keys `font.setInfo` accepts; the strict JSON Schema of each payload (every field required, optional fields nullable, no keyword that Claude's strict tool schemas refuse); the schema snapshot test |
| `error.rs` | `CommandError` and `ErrorCode`: every stable `E_*` code of M1 |
| `packed.rs` | `PackedOutline` and the binary codecs of packed outlines and `glyph.movePoints`: little-endian, validated, never panic; the shared point-flag check (also `Point::validate`) |

| Subfolder | Context |
|---|---|
| `snapshots/` | [CONTEXT.md](snapshots/CONTEXT.md): insta snapshots of the payload schemas |
