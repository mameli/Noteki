# 13: Undo and redo

**What to build:** ⌘Z undoes and ⇧⌘Z redoes, per Tab. Typing is undone a run at a time (a word, or until a pause), as macOS text views do; a paste, a cut, a checkbox click, a format command and a Merge each are one step. Undo puts the cursor where the change was. Starting from Ferrite's `src/editor/ferrite/history.rs` (MIT): operations rather than snapshots, typing runs merged; adapted to byte offsets and listed in `THIRD_PARTY.md`.

**Blocked by:** 05

**Status:** ready-for-agent

- [ ] `core::editor::history`: record, undo, redo, merge of typing runs, tested without GPUI (Ferrite's tests ported)
- [ ] Every edit of `NoteEditor` goes through it; `set_text` (opening, reload) clears it, except a Merge, which is one undoable step
- [ ] Each Tab keeps its own history when switching Tabs (SPEC › Tabs: "its own cursor, scroll and undo")
- [ ] Composed text (a dead key) is recorded once, when committed
- [ ] Robustness: random edits, undo all, gives the original text; redo all gives the last
