# 14: Format commands that toggle

**What to build:** The format commands of SPEC › Format toggle as Obsidian's do: ⌘B and ⌘I wrap the selection (or the word at the cursor) in `**`/`*`, or remove the marks if it already has them; ⌘1–⌘6 make the line a Heading of that level, or plain text if it already is one; ⌘↩ turns a line into a task, a list item into its task, and ticks or clears a task; ⌘] and ⌘[ indent and outdent; Quote, Bulleted and Numbered list from the menu. Starting from Ferrite's `src/markdown/formatting.rs` (MIT), adapted to byte offsets, listed in `THIRD_PARTY.md`. Today `wrap_selection` only adds marks.

**Blocked by:** 13

**Status:** ready-for-agent

- [ ] `core::editor::format` (or `core::markdown`): each command from text and selection to new text and selection, tested without GPUI (Ferrite's tests ported, plus SPEC's cases: "again, the heading goes")
- [ ] Every command is one undo step (ticket 13)
- [ ] ⌘↩ no longer follows the link under the cursor when it formats (SPEC binds ⌘↩ to Task; following a link moves to the click of ticket 10)
- [ ] UI tests: each shortcut on a selection and on a bare cursor, and its toggle back
