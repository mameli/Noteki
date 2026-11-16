# 03: ⌘D selects the next occurrence

**What to build:** ⌘D with the cursor in a word selects the word; again, it adds the next occurrence of the selected text as another selection (wrapping at the end of the Note), as in Obsidian and every code editor. Typing, ⌫ and pasting then act on every selection at once, as one undo step. Esc or a click goes back to one selection.

**Blocked by:** 02

**Status:** ready-for-agent

- [x] `EditBuffer` holds extra selections; an edit applies to all of them as one replacement of the span they cover, so offsets stay right and it is one undo step
- [x] Every selection drawn; the caret drawn in each when they are empty
- [x] Moving the cursor with the arrows, a click or Esc keeps only the main selection
- [x] Tests: in `core::editor::buffer` (⌘D steps, typing into three selections, ⌫ in each, undo); UI (⌘D ⌘D, type, one ⌘Z)

## Comments

Done (2026-11-17).

- `EditBuffer` keeps extra selections. `select_next_occurrence` (⌘D): the word at the cursor, then the next occurrence of the selected text (case as written, wrapping, skipping those already selected) as the new main selection. Typing and ⌫/⌦ with several selections build one replacement of the span from the first to the last selection, so it is one edit, one undo step, through the same `replace` as every edit; the selections end as carets after their new text. A composition (dead key) acts on the main selection only.
- Moving the cursor, a click, `set_selection`, select all and any single edit (format, checkbox, paste through `replace`) go back to one selection; Esc too (`SingleSelection`).
- The element draws every selection and a caret in each empty one.
- Find's "All" button (left from ticket 02) selects every match this way.
- Tests: ⌘D steps, typing and ⌫ in three selections, undo, a move back to one; robustness (⌘D among random edits: every selection on a char boundary; undo all still gives the original); UI (⌘D ×3, typing, one ⌘Z; find "All", typing, Esc). New mutant: typing with several selections changing only the main one. Killed. Timing unchanged.
