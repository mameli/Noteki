# Tables are drawn in place, row by row, not by codemirror-markdown-tables

The SPEC planned tables "via codemirror-markdown-tables". That library turns a table into an interactive component: it replaces the Markdown with an HTML table of its own height, and it autoformats and prettifies the table while the user edits, and corrects the line breaks around it. Each of these rewrites text the user did not touch, which a Merge with Takku ([ADR 0005](0005-merge-external-changes.md)) and Obsidian on the same file ([ADR 0002](0002-coexist-with-obsidian.md)) then see as edits. A block of another height also makes the text jump when the cursor enters or leaves the table, against the SPEC's Live preview.

Noteki draws a table in place instead, with the same decorations as the rest of Live preview: every row stays one line, its pipes are hidden, its cells are boxes of their column's width and alignment, and the delimiter row is a rule. The height is the same whether the table is shown or written, and the file is never changed by showing it.

## Considered options

- **codemirror-markdown-tables**: a ready table editor (sorting, cell navigation), about 500 KB unpacked with its dependencies; it rewrites tables and changes heights.
- **A block widget of our own** (an HTML table replacing the rows): a real table, but of another height, and a click lands in the widget, not in the text.

## Consequences

- Table lines never wrap, shown or written: a wide row scrolls sideways. A cell is cut at 40 characters, with an ellipsis.
- Column widths are approximate in a proportional font (characters times 0.6 em).
- Entering a table shows its source: the pipes come back and the text moves sideways under the cursor, never up or down.
