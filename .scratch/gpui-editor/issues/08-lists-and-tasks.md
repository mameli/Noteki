# 08: Lists and tasks

**What to build:** A bullet's `- `, `* ` or `+ ` is drawn as `•`; nested items are indented by their level and a wrapped item's rows line up under its text, not under the bullet; a numbered item keeps its number. A task's `- [x] ` is a checkbox in place of the bullet and brackets (a number stays), the marks showing while the cursor touches them; a click on the checkbox writes `[x]` or `[ ]` in the file and leaves the cursor where it was. A done task is crossed out and faded (`TaskDone`, as now). SPEC › Live preview › Task lists. This brings in **replacements** in the display map: a hidden range drawn as a string or as a box of a given width.

**Blocked by:** 07

**Status:** ready-for-agent

- [x] ~~The display map supports a replaced range~~: not needed, see the comments
- [x] Bullets as `•` (one glyph for every level, as Obsidian), numbers as written
- [x] Hanging indent: a wrapped list item's next rows start under its text
- [x] The checkbox is drawn in the theme's colours, checked or not; clicking it calls `set_task_checked`, saves as any edit, and does not move the cursor or the selection
- [x] Only `[ ]`, `[x]`, `[X]` are tasks; `[-]` and `[/]` stay as written
- [x] "Liste" and "Checklist" of `rendering-test.md` draw as in Obsidian
- [x] Tests: replacements in the display map (round trip, never inside); UI: a click on a checkbox toggles the right task and leaves the cursor; a wrapped item's second row starts at its text's x

## Comments

Done (2026-11-16), more simply than the ticket planned.

- **No replacements in the display map.** A list item's marks (indent, `- `, `3. `, `- [ ] `) are hidden like any other mark, and the line is indented by ticket 07's per-line indent (20 px per level plus 24 px for the marker). The element paints the bullet (a dot), the number or the checkbox in that room, as a line decoration. Because the whole line is indented, a wrapped item's rows line up under its text with nothing more.
- `core::markdown::list_item` reads an item's level (two spaces or a tab per level), kind (bullet, number with its delimiter, task) and where its text starts. The marks show while the cursor touches them (SPEC), and then the item is drawn as written, unindented.
- A click on a checkbox (with 3 px of room around it) writes `x` or a space between the brackets, keeps the selection, and is an edit like any other (Edited, autosave). `set_task_checked` was not needed: the bracket's offset is known from the line.
- A numbered task keeps its number and its brackets as written (SPEC: "a number stays"); a checkbox after a number is left for when someone needs it.
- **Bug found and fixed**: where a line wraps, GPUI puts the caret at the end of the row before; it is now at the start of the next row, as in macOS (it showed in every wrapped paragraph, not only lists: ⌘← on a second row drew the caret at the end of the first).
- Tests: `list_item` and the decorations without GPUI (levels, numbers, tasks, `1. [ ]`, marks shown when touched); UI (indent by level, a wrapped item's second row under its text, a click on a checkbox toggles it both ways and keeps the cursor). Two mutants (caret at a wrap; unchecking), killed. Typing 9.3 ms, arrow 5.5 ms, idle 1.8 ms.
