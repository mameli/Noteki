# 08: Lists and tasks

**What to build:** A bullet's `- `, `* ` or `+ ` is drawn as `•`; nested items are indented by their level and a wrapped item's rows line up under its text, not under the bullet; a numbered item keeps its number. A task's `- [ ] ` is a checkbox in place of the bullet and brackets (a number stays), the marks showing while the cursor touches them; a click on the checkbox writes `[x]` or `[ ]` in the file and leaves the cursor where it was. A done task is crossed out and faded (`TaskDone`, as now). SPEC › Live preview › Task lists. This brings in **replacements** in the display map: a hidden range drawn as a string or as a box of a given width.

**Blocked by:** 07

**Status:** ready-for-agent

- [ ] The display map supports a replaced range: hidden, with a shown string or an inline box (width, painted by the element) in its place; offsets map to before or after it, never inside
- [ ] Bullets as `•` (one glyph for every level, as Obsidian), numbers as written
- [ ] Hanging indent: a wrapped list item's next rows start under its text
- [ ] The checkbox is drawn in the theme's colours, checked or not; clicking it calls `set_task_checked`, saves as any edit, and does not move the cursor or the selection
- [ ] Only `[ ]`, `[x]`, `[X]` are tasks; `[-]` and `[/]` stay as written
- [ ] "Liste" and "Checklist" of `rendering-test.md` draw as in Obsidian
- [ ] Tests: replacements in the display map (round trip, never inside); UI: a click on a checkbox toggles the right task and leaves the cursor; a wrapped item's second row starts at its text's x
