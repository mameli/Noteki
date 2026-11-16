# 07: Horizontal rules and quotes

**What to build:** A line `---`, `***` or `___` (a thematic break, not a setext Heading's underline) is drawn as a thin rule across the text while the cursor is off it. A quote's `> ` marks are hidden and a bar is painted on its left, one per level of nesting; its text is the quote colour, as now. This brings in **line decorations**: what the element paints around a line's text without changing its height.

**Blocked by:** 06

**Status:** ready-for-agent

- [ ] `LineStyle` carries a decoration (rule, quote bar with its depth); the element paints it before the text, from the line's top and height
- [ ] A rule's characters are hidden off the cursor's line and shown on it; the line keeps its height either way
- [ ] `---` under a paragraph line is a setext Heading's underline, not a rule (SPEC › Outline already decides this; reuse it)
- [ ] A quote's bar runs the whole height of each of its lines, wrapped rows included, so consecutive quote lines draw one continuous bar; an empty `>` line keeps the bar
- [ ] The `> ` of the cursor's line shows; the other quote lines keep theirs hidden
- [ ] "Citazioni" and the two `---` of `rendering-test.md` draw as in Obsidian
- [ ] Tests: the decoration per line without GPUI (rule vs setext, nested quotes); UI: a rule line's height equal with and without the cursor
