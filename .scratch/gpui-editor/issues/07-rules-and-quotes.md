# 07: Horizontal rules and quotes

**What to build:** A line `---`, `***` or `___` (a thematic break, not a setext Heading's underline) is drawn as a thin rule across the text while the cursor is off it. A quote's `> ` marks are hidden and a bar is painted on its left, one per level of nesting; its text is the quote colour, as now. This brings in **line decorations**: what the element paints around a line's text without changing its height.

**Blocked by:** 06

**Status:** ready-for-agent

- [x] `LineStyle` carries a decoration (rule, quote bar with its depth); the element paints it before the text, from the line's top and height
- [x] A rule's characters are hidden off the cursor's line and shown on it; the line keeps its height either way
- [x] `---` under a paragraph line is a setext Heading's underline, not a rule (SPEC › Outline already decides this; reuse it)
- [x] A quote's bar runs the whole height of each of its lines, wrapped rows included, so consecutive quote lines draw one continuous bar; an empty `>` line keeps the bar
- [x] The `> ` of the cursor's line shows; the other quote lines keep theirs hidden
- [x] "Citazioni" and the two `---` of `rendering-test.md` draw as in Obsidian
- [x] Tests: the decoration per line without GPUI (rule vs setext, nested quotes); UI: a rule line's height equal with and without the cursor

## Comments

Done (2026-11-16).

- `core::markdown::lines` classifies a thematic break as `BlockKind::Rule`, unless it is a `---` under a paragraph line, which stays `Text` (a setext underline); `is_setext_candidate` moved from `index.rs` to `markdown.rs` so both use one rule. Off the cursor's line a rule's text is one `SyntaxMarker`.
- `quote_prefix` gives a quote's depth and the length of its `>` marks, hidden off the cursor's line (SPEC: a quote's `>` shows only on the cursor's line).
- `LineStyle::decoration` (`Rule`, `Quote { depth }`). The element paints it before the text: a 1 px rule in the separator colour across the text, or one 3 px bar per level in the accent colour, the line's full height (consecutive quote lines draw one bar). A quote's text is indented 16 px per level, on its line or not, so showing `> ` never moves its text. The indent is per line in the layout (positions, clicks, selection, wrapping), ready for list items' hanging indent (ticket 08).
- Tests: decorations per line (rule, setext text and underline, nested quotes, an empty `>`, `* * *`, a rule on the cursor's line); UI (a rule's height the same with the cursor on it or not; quote text indented by depth and kept there when its `> ` shows). New mutant: a rule drawn over its marks on the cursor's line. Killed.
