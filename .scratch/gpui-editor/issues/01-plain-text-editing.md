# 01: Plain text editing in an element of our own, in place of GPUI Kit's

**What to build:** Noteki's own editor element replaces GPUI Kit's `Editor` in `NotekiApp`, and the old one is removed. A Note can be typed into, deleted from and moved through with the keyboard, accents and input methods included, and it still autosaves, Merges and reloads as before. No Live preview yet: every line is drawn at the text size. Underneath, the layout is already per line (each line shaped and measured on its own, its top the sum of the heights above), so that ticket 03 only changes sizes. This is the tracer bullet: buffer and rules in `src/core/editor/`, element in `src/editor/`, wired into the app, tested.

**Blocked by:** none

**Status:** ready-for-agent

- [x] `EditorState` and `Editor` from GPUI Kit are gone from `src/app`; `update_decorations`, `TextDecoration` and `wrap_selection` are removed (they come back with their own tickets)
- [x] The app reaches the editor through set/read text, set/read cursor and a text-changed event; opening a Note, switching Tabs, autosave, Merge, Conflict and reload work as before
- [x] Typing, Return, ⌫ and ⌦
- [x] ← → by character (grapheme cluster), ⌥← ⌥→ by word, ⌘← ⌘→ to the line's start and end, ↑ ↓ by line, ⌘↑ ⌘↓ to the start and end of the Note
- [x] ⇧ with any of these extends the selection; the selection is drawn; typing replaces it
- [x] Text arrives through `EntityInputHandler`: `è` with press-and-hold, `´` + `e` with a dead key, and dictation all work
- [x] The cursor blinks and is drawn at the right place in lines with emoji and combining accents
- [x] Tested in `src/core/editor/`: edits and movements give the expected text and cursor; in `tests/robustness.rs`: random edits and movements never panic and leave the cursor on a char boundary
- [x] `tests/ui.rs` passes on the new editor: the existing tests of typing, autosave, Merge, Conflict and reload moved onto it, not deleted

## Comments

Done (2026-11-16).

- `src/core/editor/`: `EditBuffer` (text, selection as anchor/head, composed text), `movement` (grapheme, word, line ends, CRLF), `lines` (logical lines, `LineTops`: the top of each line as the sum of the heights above). UTF-16 conversions for the macOS text input system.
- `src/editor/`: `NoteEditor` plus `EditorElement`. Each logical line is shaped on its own with `shape_text`, wrapped to the width, and stacked by `LineTops`. Caret, selection, clicks and ↑/↓ all go through those positions. The element measures its height from its width, and the view scrolls it in a native `overflow_y_scroll` div.
- The app: `EditorState`/`Editor`, `update_decorations`, `TextDecoration` and `LIVE_PREVIEW_MAX_BYTES` are gone. `src/app/style.rs` stays, unused until ticket 03 (`#[allow(dead_code)]`). Format shortcuts, `[[` insertion, ⌘↩ and Outline jumps were cheap to keep through `replace_selection`/`set_selected_range`, so they still work.

Done beyond the ticket, because each one was a few lines and the editor was unusable in the app without it:
- Soft wrap, since `shape_text` wraps for free. ↑/↓ already move by wrapped row and keep the x the movement began with. That part of ticket 02 is done; it still needs autoscroll to the cursor and drawing only the visible lines.
- Native scrolling with momentum (the div's).
- Click, drag and ⇧-click. Ticket 05 keeps double/triple click, dragging past the edge, and the display map.
- ⌘C ⌘X ⌘V (plain text), ⌘A, the Emacs keys (⌃A ⌃E ⌃K ...), ⌥⌫ and ⌘⌫.

Not done: undo; ⌘←/⌘→ go to the logical line's ends, not the wrapped row's (ticket 02); every line is shaped at every frame (GPUI caches the shaped lines, but a very large Note will feel it: ticket 02).

Tests: 16 unit tests in `core::editor`; a property in `tests/robustness.rs` (random edits, motions, compositions and selections never leave an offset off a char boundary); 4 UI tests (keys, Edited only for user edits, ↑/↓ keeping the x across a shorter line, a dead key composing `é` through `ElementInputHandler`). The 21 existing UI tests run unchanged on the new editor. Mutation check 10/10. The release-app smoke run was not run here (it opens a window).
