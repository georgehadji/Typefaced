# Typefaced — instructions for coding agents

Proprietary desktop font editor: Tauri 2 shell, Rust engine, React UI. All rights reserved.

## 1. Orient yourself first
1. **Read `CONTEXT.md` in the root before you do anything else.** It explains what the
   project does, lists every tracked file and links to the `CONTEXT.md` of every folder.
2. Before you read or change files in a folder, read that folder's `CONTEXT.md`. It says
   what each file there does.
3. Find files through these maps. Search the whole tree only when the maps do not answer
   the question.

## 2. Follow the rules
**`CLAUDE.md` is binding for every agent, not only Claude.** It holds the hard rules
(clean room, license allow-list, layering, typed commands, secrets, test first, git),
the commands and the CI jobs. Read it after `CONTEXT.md`.

## 3. Keep the maps true
- When you add, remove, rename or change the role of a file, update that folder's
  `CONTEXT.md` in the same commit. A new folder gets its own `CONTEXT.md` and a row in the
  root `CONTEXT.md`.
- Do not add `CONTEXT.md` to `apps/desktop/src-tauri/capabilities/` or inside UFO
  packages (`*.ufo/`): tools read every file there. The parent folder describes them.
- If a `CONTEXT.md` disagrees with the code, the code wins: fix the `CONTEXT.md`.
