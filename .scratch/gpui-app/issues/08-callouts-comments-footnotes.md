# 08: Callouts, Comments, footnotes, setext Headings

**What to build:** As SPEC › Live preview says: a callout `> [!type] Title` is a box in its type's colour with its title (the type with a capital when none), its `>` and `[!type]` marks shown only on the cursor's line; a Comment `%% … %%` is faded with its `%%` hidden (done) and spans lines; a footnote reference `[^label]` is a small raised number (in order of first appearance), its definition `[^label]:` shows that number, a click on either goes to the other; a setext Heading (a paragraph line underlined with `===` or `---`) is drawn as a Heading of level 1 or 2, its underline hidden off the cursor.

**Blocked by:** none

**Status:** ready-for-agent

- [x] Callout box as a line decoration, Obsidian's types and colours mapped onto the theme
- [x] Footnote labels (not raised: see the comments) and the click between reference and definition
- [x] Setext Headings sized like ATX ones; the Outline already finds them
- [x] Tests: per-line styles without GPUI; UI on a sample with each

## Comments

Done (2026-11-17), checked in the real app with a sample (callouts of three types, a setext Heading, inline and multi-line Comments, a footnote).

- **Callouts**: `callout_marks` reads `[!type]` (with `-`/`+`); off the cursor's line the `> [!type] ` marks are hidden and the title shows, or the type when there is none (as written: GPUI cannot capitalize a letter in place, so `tip` stays `tip`). A callout is the header and the quote lines under it, drawn as a box tinted in its type's colour with a bar on the left (Obsidian's types and aliases: blue, green, orange, red, purple, grey from the theme's colours); the title is semibold. The box is the block's, so it stays while the cursor is on one of its lines.
- **Comments across lines**: from a line with an unpaired `%%` to the next; the lines between are faded, the opening and closing `%%` hidden.
- **Footnotes**: `[^label]` and `[^label]:` at a line's start: marks hidden off the element, the label in the link colour. GPUI cannot raise or shrink a part of a line, so it is not superscript, and the label is shown as written rather than renumbered. A click on a reference moves the cursor to its definition's text, on a definition's label to its first reference.
- **Setext Headings**: sized and styled as a Heading of level 1 (`===`) or 2 (`---`); the underline stays as written (SPEC), without inline styles (`====` is no highlight). The cursor's line keeps its Heading size through the cursor pass.
- Bug seen in the screenshot and fixed: the underline was styled as a highlight because its spans were cleared before they were added.
- Tests: callout marks and footnotes, their targets, callout colours and headers, setext levels, Comments across lines, an untitled callout, the underline without styles; UI (setext size, callout indent, a click on a reference). New mutant: a warning callout coloured as a note. Killed.
