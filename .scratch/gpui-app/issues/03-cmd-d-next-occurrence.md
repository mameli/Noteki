# 03: ⌘D selects the next occurrence

**What to build:** ⌘D with the cursor in a word selects the word; again, it adds the next occurrence of the selected text as another selection (wrapping at the end of the Note), as in Obsidian and every code editor. Typing, ⌫ and pasting then act on every selection at once, as one undo step. Esc or a click goes back to one selection.

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] `EditBuffer` holds extra selections; an edit applies to all of them from the last to the first, so offsets stay right; one undo step
- [ ] Every selection drawn; the caret drawn in each when they are empty
- [ ] Moving the cursor with the arrows, a click or Esc keeps only the main selection
- [ ] Tests: in `core::editor::buffer` (⌘D steps, typing into three selections, ⌫ in each, undo); UI (⌘D ⌘D, type, one ⌘Z)
