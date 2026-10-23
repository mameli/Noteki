# 20: Tables

**What to build:** Tables render as tables (drawn in place, ADR 0008) while the cursor is outside, and become source when it enters.

**Blocked by:** 19 (Live preview of inline syntax)

**Status:** ready-for-agent

- [x] Tables render with header and alignment
- [x] Entering a table shows its source; leaving renders it again
- [x] The text does not jump when it switches
- [x] Tested at Seam 2

## Comments

Done, without codemirror-markdown-tables (ADR 0008). That library replaces the table with an interactive block of another height, and reformats and prettifies tables while the user edits, rewriting text a Merge with Takku and Obsidian would then see as edits. Tables are drawn in place instead (`tables.ts`):
- every row stays one line, pipes hidden;
- each cell is a box as wide as its column's widest cell as shown (up to 40 characters), aligned as the delimiter row says;
- the delimiter row is a rule; the header has a background;
- an empty cell keeps its place; an escaped `\|` shows without its backslash;
- a body row's cells past the header's are not shown, as in GFM.

A table is source while a cursor or a selection's end is in it (⌘A leaves tables drawn). Table lines never wrap, drawn or written, so the height is the same either way: a wide row scrolls sideways. Entering a table brings the pipes back, so the text moves sideways, never up or down.

Seam 2: 9 table tests (19 in all with Live preview).

Fixed after review: long rows wrapped as source but not when drawn (height changed), rows in a quote or list lost their styling, a table across two visible ranges was drawn twice, header and body columns did not line up (ch in bold), `||` shifted the following cells, widths changed while scrolling, an escaped pipe in a Wikilink was hidden twice.

Verified on the editor page in a browser: tables drawn with alignment, a long row and a table in a quote; the line below the table stays at the same height with the cursor outside, inside and after leaving it, with no errors. Not checked in the app window: macOS would not bring it forward while the user was working, and the window was not reachable; it is the same editor page.
