# 13: Undo and redo

**What to build:** ⌘Z undoes and ⇧⌘Z redoes, per Tab. Typing is undone a run at a time (a word, or until a pause), as macOS text views do; a paste, a cut, a checkbox click, a format command and a Merge each are one step. Undo puts the cursor where the change was. Starting from Ferrite's `src/editor/ferrite/history.rs` (MIT): operations rather than snapshots, typing runs merged; adapted to byte offsets and listed in `THIRD_PARTY.md`.

**Blocked by:** 05

**Status:** ready-for-agent

- [x] `core::editor::history`: record, undo, redo, merge of typing runs, tested without GPUI (written, not ported: see the comments)
- [x] Every edit of `NoteEditor` goes through it; `set_text` (opening, reload) clears it, except a Merge, which is one undoable step
- [x] Each Tab keeps its own history when switching Tabs (SPEC › Tabs: "its own cursor, scroll and undo")
- [x] Composed text (a dead key) is recorded once, when committed
- [x] Robustness: random edits, undo all, gives the original text; redo all gives the last

## Comments

Done (2026-11-16), **without Ferrite's code**: its `history.rs` is 1,549 lines built for Ferrite's two editors (snapshots per frame, anomaly guards, metadata). Noteki's buffer has one place where the text changes (`EditBuffer::replace`), so a history of its own is about a hundred lines: nothing to list in `THIRD_PARTY.md`.

- `core::editor::history`: steps of replacements (`Edit`: at, removed, inserted) with the selection before and after. A run of typing (one character at a time), a run of ⌫ or ⌦, or a composition (a dead key, an input method) is one step until the cursor moves (`move_to`, `set_selection`, `select_all` close the run) or another kind of edit comes. Return, a paste, a cut, a format command and a checkbox click are steps of their own (`close_undo_step`). At most 1,000 steps.
- `EditBuffer::replace_all` turns a reload or a Merge into one undoable step (the one replacement between the old and new text, `diff`). `NoteEditor::open_note` keeps each Note's history while another shows (SPEC › Tabs: each its own undo); the app calls it when the editor shows another Note, and `set_text` for a reload.
- ⌘Z and ⇧⌘Z in the editor; undoing is an edit for the app (Edited, autosave).
- Tests: steps, runs, a dead key, redo dropped by a new edit, `diff`; robustness (random edits, motions and compositions: undo all gives the original text, redo all the last); UI (a run undone and redone, each Tab its own undo, a checkbox click undone alone). Mutant: typing after a cursor move joining the run before. Killed. A first mutant (typing not adjacent to the run joining it) survived because moving the cursor already closes the run, so that adjacency check was dead code and was removed.
