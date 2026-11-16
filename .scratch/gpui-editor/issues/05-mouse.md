# 05: Click, drag and select with the mouse

**What to build:** A click puts the cursor at the offset under the pointer, on lines of any height and with hidden marks. Dragging selects, ⇧-click extends the selection, a double-click selects a word, a triple-click the line. Dragging past the top or bottom edge scrolls.

**Blocked by:** 04

**Status:** ready-for-agent

- [x] A click on shown text lands at the right offset in the Note (through the display map); a click on a Heading's line puts the cursor before the character that was clicked, even though its `#`s then appear and shift the line to the right
- [x] A click past the end of a line puts the cursor at its end; below the last line, at the end of the Note
- [x] Drag, ⇧-click, double-click (word), triple-click (line)
- [x] Dragging past the edge of the view scrolls it, faster the further the pointer is
- [x] The pointer is an I-beam over text
- [x] Headless test: a click at a position on a Heading and on a wrapped line gives the expected cursor

## Comments

Done (2026-11-16).

- **Click**: through the display map, so a click on a Heading whose `# ` is hidden lands before the letter clicked, though the `# ` then appears and shifts the line. Past a line's end goes to its end; below the text, to the end of the Note (done in ticket 01, now tested).
- **Double-click** selects the word (`core::editor::movement::word_range_at`: letters, digits and `_`, accented letters included), or the run of spaces, or the one punctuation mark under the pointer. **Triple-click** selects the logical line with its line break (`line_range_at`). Dragging after a double- or triple-click goes on selecting by words or lines, always keeping what the click selected.
- **⇧-click** extends the selection from its anchor.
- **Dragging past the top or bottom** scrolls, faster the further the pointer is (`core::editor::lines::edge_scroll`, at most 60 px per 16 ms), extending the selection with the text that comes into view, until the button is released.
- **Bug found by the tests**: GPUI sends `on_mouse_move` to an element only while the pointer is over it, so a drag stopped following the pointer once it left the editor (over the tab bar, the status bar, past the window). The element now listens to the whole window while a drag is on (`window.on_mouse_event` in `paint`).
- The pointer is an I-beam over the editor (since ticket 01).

Tests: unit tests for `word_range_at`, `line_range_at` and `edge_scroll`; 6 UI tests with real mouse events (a click on a hidden-mark Heading, past a line and below the text, on a wrapped row, double and triple click, drag and ⇧-click, dragging below the view scrolling and stopping on release). New mutant: the drag no longer followed outside the editor. Killed. Mutation check 18/18.
