# 14: Format commands that toggle

**What to build:** The format commands of SPEC › Format toggle as Obsidian's do: ⌘B and ⌘I wrap the selection (or the word at the cursor) in `**`/`*`, or remove the marks if it already has them; ⌘1–⌘6 make the line a Heading of that level, or plain text if it already is one; ⌘↩ turns a line into a task, a list item into its task, and ticks or clears a task; ⌘] and ⌘[ indent and outdent; Quote, Bulleted and Numbered list from the menu. Starting from Ferrite's `src/markdown/formatting.rs` (MIT), adapted to byte offsets, listed in `THIRD_PARTY.md`. Today `wrap_selection` only adds marks.

**Blocked by:** 13

**Status:** ready-for-agent

- [x] `core::editor::format`: each command from text and selection to new text and selection, tested without GPUI (written, not ported: see the comments), with SPEC's cases ("again, the heading goes")
- [x] Every command is one undo step (ticket 13)
- [x] ⌘↩ no longer follows the link under the cursor when it formats (SPEC binds ⌘↩ to Task; following a link moves to the click of ticket 10)
- [x] UI tests: each shortcut on a selection and on a bare cursor, and its toggle back

## Comments

Done (2026-11-16), **without Ferrite's code**: its `formatting.rs` is 1,289 lines serving its WYSIWYG mode too. The commands are a few pure functions on the text here (`core::editor::format`, about 250 lines with tests).

- `toggle_wrap` (⌘B, ⌘I, ⌘⇧X, ⌃\`): wraps the selection, or the word at the cursor, or puts both marks with the cursor between; takes the marks away when they are around the selection or selected with it. `*` and `**` tell their stars apart by counting them (one or three: italic; two or three: bold).
- `format_lines` (⌘1–⌘6, ⌘↩, ⌘], ⌘[, and Quote, Bulleted list, Numbered list in the command palette): on every line the selection touches. Again, a Heading of the same level goes; another level replaces it. ⌘↩ makes a line a task, gives a list item its box, ticks and clears a task. A cursor keeps its place in its line's text.
- Each command is one replacement, one undo step (`NoteEditor::format`). The app's ⌘B... actions call it; ⌘↩ no longer follows the link under the cursor (that stays in the command palette, and a click opens links since ticket 10). ⌘[ and ⌘] indent and outdent in the editor; elsewhere they stay Back and Forward.
- Tests: wraps and their toggles, italic around bold, an empty place, Headings (toggle, level change, the cursor's place), tasks, lists, quotes, indent and outdent; UI (⌘B on the word at the cursor and back, one undo step; ⌘2, ⌘↩ twice, ⌘], ⌘[, three undo steps). New mutant: ⌘I on bold text taking a star away. Killed. Mutation check 28/28.
