# 12: Tables drawn in place

**What to build:** A table is drawn as a grid in place, as ADR 0008 says: every row stays one line, its pipes are hidden, its cells are boxes of their column's width and alignment (`:---`, `:---:`, `---:`), the header bold over the delimiter row drawn as a rule. A table wider than the text has its columns shrunk to fit, their text cut with an ellipsis. In this ticket the row the cursor is on shows as source; editing cell by cell (Tab, ↑/↓ between cells, ADR 0008) is a later ticket.

**Blocked by:** 08

**Status:** ready-for-agent

- [ ] Column widths from the widest cell of each column, up to 40 characters, measured with the text system (not characters × 0.6 em, which ADR 0008 accepted only for CodeMirror)
- [ ] The grid is line decorations plus replacements (each `|` replaced by a cell boundary), so a row keeps its height and nothing moves when the cursor comes and goes
- [ ] Inline Live preview inside cells (bold, code, links), escaped `\|` kept in its cell
- [ ] "Tabelle" of `rendering-test.md` draws with its three alignments
- [ ] Tests: column widths and alignment without GPUI; UI: entering and leaving the table moves no line above it
