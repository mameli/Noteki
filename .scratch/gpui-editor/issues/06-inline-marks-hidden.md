# 06: Inline marks hidden, shown inside the element the cursor is in

**What to build:** Bold, italic, bold italic (`***`), strikethrough, highlights, inline code, links and Wikilinks are drawn without their marks: `**`, `*`, `~~`, `==`, the backticks, `[`, `](url)`, `[[`, `]]` and a Wikilink's target before its `|` Alias. A Comment's `%%` too. The marks of one element show while the cursor (or the end of the selection) is inside it or at its edge, only on the cursor's line, as SPEC › Live preview says; the other marks of that line stay hidden. Today every mark of the cursor's line shows: that changes.

**Blocked by:** 04

**Status:** ready-for-agent

- [ ] The scanner gives each inline element its range and its marks' ranges (a new function in `core::markdown`, or `compute_markdown_spans` extended), tested on its own: nested emphasis, `***x***`, a link with a title, `[[Note|Alias]]`, an escaped `\|` in a table, code containing `**`
- [ ] `style_lines` hides the marks of every element the cursor is not in; inline code keeps its background, a link its colour and underline
- [ ] The cursor entering an element shows its marks, leaving it hides them; never a jump on the lines above (SPEC)
- [ ] The cursor never stands in a hidden mark: arriving at an element's edge shows its marks, so → through `**a**` steps over every character as written, as in Obsidian
- [ ] Every section of `rendering-test.md` up to "Liste" reads as in Obsidian with the cursor elsewhere
- [ ] Robustness: for random texts and cursors, hidden ranges are valid, disjoint and never hold the cursor; the display map round-trips
- [ ] UI test on `rendering-test.md`: the cursor goes down line by line through the whole Note and no line above it moves
