# 12: Tables drawn in place

**What to build:** A table is drawn as a grid in place, as ADR 0008 says: every row stays one line, its pipes are hidden, its cells are boxes of their column's width and alignment (`:---`, `:---:`, `---:`), the header bold over the delimiter row drawn as a rule. A table wider than the text has its columns shrunk to fit, their text cut with an ellipsis. In this ticket the row the cursor is on shows as source; editing cell by cell (Tab, ↑/↓ between cells, ADR 0008) is a later ticket.

**Blocked by:** 08

**Status:** ready-for-agent

- [x] Column widths from the widest cell of each column, up to 40 characters, measured with the text system (not characters × 0.6 em, which ADR 0008 accepted only for CodeMirror)
- [x] The grid is line decorations plus cells placed in their columns (no replacements needed), so a row keeps its height and nothing moves when the cursor comes and goes
- [x] Inline Live preview inside cells (bold, code, links), escaped `\|` kept in its cell
- [x] "Tabelle" of `rendering-test.md` draws with its three alignments
- [x] Tests: column widths and alignment without GPUI; UI: entering and leaving the table moves no line above it

## Comments

Done (2026-11-16).

- `core::editor::table::tables`: a table is a header row, a delimiter row with as many cells (`:---`, `:---:`, `---:`), and the rows with a pipe under them; cells are the text between unescaped pipes, trimmed, outer pipes optional, a body row's cells past the header's dropped. Not in code. Every row's decoration holds the whole `Table` (its cells' text), so a change to any cell restyles, and reshapes, every row of that table and no other line.
- Off the cursor, a row hides everything but its cells' text (pipes, spaces) and the cells' inline marks; the delimiter row hides all and is drawn as a rule. The cursor's row is restyled as plain source (as every cursor line is), so it shows as written; the other rows stay a grid.
- The element shapes a table row cell by cell: column widths from each column's widest cell (the header in bold) up to 22 em (about 40 characters), padding 8 px, all shrunk in proportion when the table is wider than the text; cells aligned left, centre or right; text cut at the column's edge (clipped; no ellipsis yet). Rows never wrap and keep the text's line height, so nothing moves when the cursor comes and goes. The grid is one-pixel lines in the separator colour. Clicks and caret positions go through the cells.
- Speed: typing +0.3 ms (finding tables on every edit); arrow keys and idle frames unchanged (9.5 / 5.4 / 1.74 ms in the harness).
- Not done: the ellipsis on a cut cell, and ADR 0008's rule that only the widest columns shrink; editing cell by cell (a later ticket).
- Tests: `tables` (rows, alignments, an escaped pipe, no table without a delimiter row, in code, or with a column count that differs, outer pipes optional); hidden ranges of a row; UI on the test note's table (every row one line, left/right/centre alignment, the cursor's row as source with nothing changing height). The robustness property now models the editor's two passes (all lines with the cursor off, then the cursor's line) and fuzzes tables, tasks, Embeds and fences. New mutant: a right-aligned column drawn left. Killed.
