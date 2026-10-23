import type { EditorState } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";

// Tables in Live preview (SPEC, Live preview; ADR 0008): drawn in place, row by row. Every row
// stays one line, its pipes hidden and its cells side by side in columns, and the delimiter row a
// rule: the height is the same whether the table is shown or written, so nothing jumps when the
// cursor enters or leaves it.

export interface Span {
  from: number;
  to: number;
}

export type Align = "left" | "center" | "right";

/** A cell: where its text is, and the text. An empty one keeps one character to be drawn in. */
export interface Cell extends Span {
  text: string;
}

export interface TableRow {
  /** Where its line starts: rows of a table in a quote or a list start after its marks. */
  lineStart: number;
  row: Span;
  kind: "header" | "rule" | "body";
  cells: Cell[];
}

/** A table shown as a table: its columns, and its rows line by line. */
export interface RenderedTable extends Span {
  columns: { width: number; align: Align }[];
  rows: TableRow[];
}

/** A column is as wide as its widest cell as shown, up to this many characters. */
export const widestColumn = 40;

/** A table's rows, cells and columns, from the parser's nodes; the widths are measured after. */
export function tableLayout(state: EditorState, table: SyntaxNode): RenderedTable {
  const rows: TableRow[] = [];
  let aligns: Align[] = [];
  for (let child = table.firstChild; child; child = child.nextSibling) {
    const row = { from: child.from, to: child.to };
    const lineStart = state.doc.lineAt(child.from).from;
    if (child.name === "TableDelimiter") {
      // The delimiter row, one node for the parser: `:--` left, `:-:` centre, `--:` right.
      aligns = cells(state, row, pipes(state, row)).map(({ text }) =>
        text.startsWith(":") && text.endsWith(":") ? "center" : text.endsWith(":") ? "right" : "left",
      );
      rows.push({ lineStart, row, kind: "rule", cells: [] });
    } else if (child.name === "TableHeader" || child.name === "TableRow") {
      const pipeNodes = child.getChildren("TableDelimiter").map(({ from, to }) => ({ from, to }));
      rows.push({ lineStart, row, kind: child.name === "TableHeader" ? "header" : "body", cells: cells(state, row, pipeNodes) });
    }
  }
  // As many columns as the header: as in GFM and Obsidian, a body row's cells past them are
  // not shown, and a row with fewer ends early.
  const count = rows.find((row) => row.kind === "header")?.cells.length ?? 0;
  const columns = Array.from({ length: count }, (_, index) => ({ width: 0, align: aligns[index] ?? "left" }));
  for (const row of rows) {
    if (row.cells.length > count) row.cells = row.cells.slice(0, count);
  }
  return { from: table.from, to: table.to, columns, rows };
}

/**
 * What a row hides: everything but its cells (its pipes and the spaces around them, and cells
 * past the header's), and in its cells the backslash of an escaped pipe `\|`. The delimiter row
 * hides all its text.
 */
export function rowHidden(state: EditorState, row: TableRow): Span[] {
  const hidden: Span[] = [];
  let at = row.row.from;
  for (const cell of row.cells) {
    if (cell.from > at) hidden.push({ from: at, to: cell.from });
    const text = state.doc.sliceString(cell.from, cell.to);
    for (let index = text.indexOf("\\|"); index !== -1; index = text.indexOf("\\|", index + 2)) {
      hidden.push({ from: cell.from + index, to: cell.from + index + 1 });
    }
    at = Math.max(at, cell.to);
  }
  if (row.row.to > at) hidden.push({ from: at, to: row.row.to });
  return hidden;
}

/** Sets each column's width: its widest cell as shown, what `hidden` (in order) takes out not counted. */
export function measure(table: RenderedTable, hidden: Span[]) {
  for (const row of table.rows) {
    row.cells.forEach((cell, column) => {
      const width = Math.min(widestColumn, cell.text.length - hiddenWithin(hidden, cell));
      table.columns[column].width = Math.max(table.columns[column].width, width);
    });
  }
}

/** How much of `span` the spans in `hidden`, in order and not overlapping, cover. */
function hiddenWithin(hidden: Span[], span: Span): number {
  // The first hidden span that ends after the start of `span`.
  let low = 0;
  let high = hidden.length;
  while (low < high) {
    const middle = (low + high) >> 1;
    if (hidden[middle].to <= span.from) low = middle + 1;
    else high = middle;
  }
  let covered = 0;
  for (let index = low; index < hidden.length && hidden[index].from < span.to; index++) {
    covered += Math.min(hidden[index].to, span.to) - Math.max(hidden[index].from, span.from);
  }
  return covered;
}

/**
 * The cells of a row between its pipes, their text trimmed. A pipe at the start or the end of
 * the row opens or closes it. An empty cell keeps one character to be drawn in: a space, or the
 * pipe that ends it, made invisible, between two pipes `||`.
 */
function cells(state: EditorState, row: Span, pipes: Span[]): Cell[] {
  const bounds: Span[] = [];
  let start = row.from;
  for (const pipe of pipes) {
    bounds.push({ from: start, to: pipe.from });
    start = pipe.to;
  }
  bounds.push({ from: start, to: row.to });
  if (pipes.length && pipes[0].from === row.from) bounds.shift();
  if (pipes.length && pipes[pipes.length - 1].to === row.to) bounds.pop();
  return bounds.map((bound) => {
    const text = state.doc.sliceString(bound.from, bound.to);
    const content = text.trim();
    if (content === "") return { from: bound.from, to: bound.from + 1, text: "" };
    const lead = text.length - text.trimStart().length;
    return { from: bound.from + lead, to: bound.from + lead + content.length, text: content };
  });
}

/** The pipes of the delimiter row, which the parser does not give. */
function pipes(state: EditorState, row: Span): Span[] {
  const text = state.doc.sliceString(row.from, row.to);
  const found: Span[] = [];
  for (let index = text.indexOf("|"); index !== -1; index = text.indexOf("|", index + 1)) {
    found.push({ from: row.from + index, to: row.from + index + 1 });
  }
  return found;
}
