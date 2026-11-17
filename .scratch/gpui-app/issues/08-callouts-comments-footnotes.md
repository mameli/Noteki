# 08: Callouts, Comments, footnotes, setext Headings

**What to build:** As SPEC › Live preview says: a callout `> [!type] Title` is a box in its type's colour with its title (the type with a capital when none), its `>` and `[!type]` marks shown only on the cursor's line; a Comment `%% … %%` is faded with its `%%` hidden (done) and spans lines; a footnote reference `[^label]` is a small raised number (in order of first appearance), its definition `[^label]:` shows that number, a click on either goes to the other; a setext Heading (a paragraph line underlined with `===` or `---`) is drawn as a Heading of level 1 or 2, its underline hidden off the cursor.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Callout box as a line decoration, Obsidian's types and colours mapped onto the theme
- [ ] Footnote numbers and the click between reference and definition
- [ ] Setext Headings sized like ATX ones; the Outline already finds them
- [ ] Tests: per-line styles without GPUI; UI on a sample with each
