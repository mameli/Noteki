import { syntaxTree } from "@codemirror/language";
import { EditorState, Prec, type TransactionSpec } from "@codemirror/state";
import { type EditorView, keymap } from "@codemirror/view";
import type { SyntaxNode } from "@lezer/common";
import type { Span } from "./text";

// Tables in Live preview (SPEC, Live preview; ADR 0008): drawn in place, row by row. Every row
// stays one line, its pipes hidden and its cells side by side in columns, and the delimiter row a
// rule: the height is the same whether the table is shown or written, so nothing jumps when the
// cursor enters or leaves it.

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
  /** The cells a cursor is in, by row (in `rows`) and column: they show all their text. */
  editing?: { row: number; column: number }[];
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

/**
 * The widths the columns are drawn at, in em, from their natural widths: all of them when they
 * fit in `available`. Otherwise the narrow columns keep theirs and only the widest shrink, to one
 * width, until the table fits: a short `Repo` stays whole, a long `Perché` is cut. Without a width
 * to fit in (none measured yet), the natural widths.
 */
export function fitColumns(natural: number[], available: number): number[] {
  const total = natural.reduce((sum, width) => sum + width, 0);
  if (!(available > 0) || total <= available) return natural;
  // The cap: the widest width every column can have so that the table is `available` wide.
  const sorted = [...natural].sort((a, b) => a - b);
  let left = available;
  for (let index = 0; index < sorted.length; index++) {
    const share = left / (sorted.length - index);
    if (sorted[index] > share) return natural.map((width) => Math.min(width, share));
    left -= sorted[index];
  }
  return natural;
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

// Editing a table drawn (SPEC, Live preview): the table stays drawn with the cursor in it, as in
// Obsidian, and the cell the cursor is in shows all its text, wrapped in its column. The cursor
// always stands in a cell, never on a hidden pipe nor on the delimiter row; Tab, ⇧Tab, the arrows
// and Return go from cell to cell, and Tab after the last cell, or Return on the last row, adds a
// row.

/** Where a position is in a table drawn: its row (in `table.rows`) and column. */
export interface TablePlace {
  table: RenderedTable;
  row: number;
  column: number;
}

/** The table `pos` is in, from the syntax tree; null outside any. */
export function tableNodeAt(state: EditorState, pos: number): SyntaxNode | null {
  for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, -1); node; node = node.parent) {
    if (node.name === "Table") return node;
  }
  for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, 1); node; node = node.parent) {
    if (node.name === "Table") return node;
  }
  return null;
}

/**
 * The cell of `table` at `pos`: on a row, the cell holding it or the nearest; on the delimiter
 * row, the header's cell above it, by the pipes before. Null off the table's rows.
 */
export function placeIn(state: EditorState, table: RenderedTable, pos: number): TablePlace | null {
  const lineStart = state.doc.lineAt(pos).from;
  const index = table.rows.findIndex((row) => row.lineStart === lineStart);
  if (index === -1) return null;
  const row = table.rows[index];
  if (row.kind === "rule") {
    const header = table.rows.findIndex((candidate) => candidate.kind === "header");
    if (header === -1) return null;
    const before = state.doc.sliceString(row.row.from, Math.max(row.row.from, pos));
    const pipes = before.split("|").length - 1 - (before.trimStart().startsWith("|") ? 1 : 0);
    return { table, row: header, column: Math.max(0, Math.min(pipes, table.rows[header].cells.length - 1)) };
  }
  if (row.cells.length === 0) return null;
  let column = 0;
  let distance = Infinity;
  row.cells.forEach((cell, at) => {
    const away = pos < cell.from ? cell.from - pos : pos > cell.to ? pos - cell.to : 0;
    if (away < distance) [column, distance] = [at, away];
  });
  return { table, row: index, column };
}

/** Where the cursor goes in a cell: after its text; in an empty one, after the space it keeps. */
export function cursorIn(state: EditorState, cell: Cell): number {
  // Between two pipes `||`, the cell's one character is the pipe that ends it: before it.
  return cell.text === "" && state.doc.sliceString(cell.from, cell.to) === "|" ? cell.from : cell.to;
}

/** The place of the cursor in a table drawn, with its layout; null when it is in none. */
function placeAtCursor(state: EditorState): TablePlace | null {
  const { main } = state.selection;
  if (!main.empty) return null;
  const node = tableNodeAt(state, main.head);
  return node ? placeIn(state, tableLayout(state, node), main.head) : null;
}

/** The rows that hold cells, in order: the header and the body, not the delimiter row. */
function cellRows(table: RenderedTable): number[] {
  return table.rows.flatMap((row, index) => (row.kind === "rule" ? [] : [index]));
}

/** A new empty row after the table's last, for `columns` columns, and the cursor in its `column`. */
function addRow(state: EditorState, table: RenderedTable, column: number): TransactionSpec {
  const last = table.rows[table.rows.length - 1];
  const line = state.doc.lineAt(last.row.to);
  const prefix = state.doc.sliceString(line.from, last.row.from);
  const columns = table.columns.length;
  const insert = `\n${prefix}|${"  |".repeat(columns)}`;
  // The cursor after the first space of the cell: `| x |` as the user types.
  const cursor = line.to + 1 + prefix.length + 1 + column * 3 + 1;
  return { changes: { from: line.to, insert }, selection: { anchor: cursor }, scrollIntoView: true, userEvent: "input" };
}

function moveTo(view: EditorView, place: TablePlace, row: number, column: number): boolean {
  const cell = place.table.rows[row].cells[column];
  if (!cell) return false;
  view.dispatch({ selection: { anchor: cursorIn(view.state, cell) }, scrollIntoView: true, userEvent: "select" });
  return true;
}

/** Tab: the next cell, the first of the next row after the last; after the table's last, a new row. */
export function nextCell(view: EditorView): boolean {
  const place = placeAtCursor(view.state);
  if (!place) return false;
  const rows = cellRows(place.table);
  const columns = place.table.columns.length;
  if (place.column + 1 < Math.min(columns, place.table.rows[place.row].cells.length)) return moveTo(view, place, place.row, place.column + 1);
  const next = rows[rows.indexOf(place.row) + 1];
  if (next !== undefined) return moveTo(view, place, next, 0);
  view.dispatch(addRow(view.state, place.table, 0));
  return true;
}

/** ⇧Tab: the cell before, the last of the row above at a row's first; nothing before the first. */
export function previousCell(view: EditorView): boolean {
  const place = placeAtCursor(view.state);
  if (!place) return false;
  if (place.column > 0) return moveTo(view, place, place.row, place.column - 1);
  const rows = cellRows(place.table);
  const previous = rows[rows.indexOf(place.row) - 1];
  if (previous === undefined) return true;
  return moveTo(view, place, previous, place.table.rows[previous].cells.length - 1);
}

/** Return: the cell below, a new row under the last. */
export function cellBelowOrNewRow(view: EditorView): boolean {
  const place = placeAtCursor(view.state);
  if (!place) return false;
  const rows = cellRows(place.table);
  const next = rows[rows.indexOf(place.row) + 1];
  if (next !== undefined) return moveTo(view, place, next, Math.min(place.column, place.table.rows[next].cells.length - 1));
  view.dispatch(addRow(view.state, place.table, place.column));
  return true;
}

/**
 * ↑ and ↓: within a cell wrapped on several lines, line by line; past its edge, the cell above or
 * below, over the delimiter row; past the table's edge, out of it as anywhere.
 */
export function verticalCell(forward: boolean) {
  return (view: EditorView): boolean => {
    const place = placeAtCursor(view.state);
    if (!place) return false;
    const cell = place.table.rows[place.row].cells[place.column];
    const moved = view.moveVertically(view.state.selection.main, forward);
    if (cell && moved.head >= cell.from && moved.head <= cell.to && moved.head !== view.state.selection.main.head) return false;
    const rows = cellRows(place.table);
    const target = rows[rows.indexOf(place.row) + (forward ? 1 : -1)];
    if (target === undefined) {
      // Out of the table: from the line after its last row, or before its first.
      const edge = view.state.doc.lineAt(forward ? place.table.to : place.table.from);
      const beyond = forward ? edge.to + 1 : edge.from - 1;
      if (beyond < 0 || beyond > view.state.doc.length) return true;
      view.dispatch({ selection: { anchor: beyond }, scrollIntoView: true, userEvent: "select" });
      return true;
    }
    return moveTo(view, place, target, Math.min(place.column, place.table.rows[target].cells.length - 1));
  };
}

/** ⌫ at a cell's start and ⌦ at its end take nothing: they would take a pipe and join two cells. */
export function keepCellEdge(forward: boolean) {
  return (view: EditorView): boolean => {
    const place = placeAtCursor(view.state);
    if (!place) return false;
    const cell = place.table.rows[place.row].cells[place.column];
    if (!cell) return false;
    const head = view.state.selection.main.head;
    if (cell.text === "") return true;
    return forward ? head >= cell.to : head <= cell.from;
  };
}

/**
 * The cursor never stays where the table hides text (a pipe and its spaces) nor on the delimiter
 * row, which the header covers: there it goes into the nearest cell, so what is typed lands in a
 * cell. A click on the header's lower half lands on the delimiter row.
 */
export const cursorInCells = EditorState.transactionFilter.of((transaction) => {
  if (!transaction.selection || transaction.selection.ranges.length !== 1 || !transaction.selection.main.empty) return transaction;
  const state = transaction.state;
  const head = transaction.selection.main.head;
  const node = tableNodeAt(state, head);
  if (!node) return transaction;
  const layout = tableLayout(state, node);
  const place = placeIn(state, layout, head);
  if (!place) return transaction;
  const row = layout.rows[place.row];
  const cell = row.cells[place.column];
  if (!cell) return transaction;
  const onRule = state.doc.lineAt(head).from !== row.lineStart;
  if (!onRule && head >= cell.from && head <= cell.to) return transaction;
  const anchor = onRule ? cursorIn(state, cell) : head < cell.from ? cell.from : cell.to;
  return [transaction, { selection: { anchor }, sequential: true }];
});

export const tableKeys = Prec.high(
  keymap.of([
    { key: "Tab", run: nextCell, shift: previousCell },
    { key: "Enter", run: cellBelowOrNewRow },
    { key: "ArrowDown", run: verticalCell(true) },
    { key: "ArrowUp", run: verticalCell(false) },
    { key: "Backspace", run: keepCellEdge(false) },
    { key: "Delete", run: keepCellEdge(true) },
  ]),
);
