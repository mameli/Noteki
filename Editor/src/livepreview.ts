import { syntaxTree } from "@codemirror/language";
import type { EditorState, Range } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import type { SyntaxNode } from "@lezer/common";
import { embeddedFile } from "./embeds";
import { isUnresolved } from "./wikilink";
import { measure, type RenderedTable, rowHidden, type Span, tableLayout } from "./tables";

export type { RenderedTable } from "./tables";

// Live preview (SPEC, Live preview): bold, italic, strikethrough, inline code, links, Wikilinks
// and Headings show without their marks, and the marks of an element appear while the cursor is in
// it, only on the cursor's line; tables are drawn as tables while the cursor is out of them
// (tables.ts). What is shown above the cursor never changes as it moves, so lines above it never
// change height.

const emphasis: Record<string, string> = {
  Emphasis: "EmphasisMark",
  StrongEmphasis: "EmphasisMark",
  Strikethrough: "StrikethroughMark",
  InlineCode: "CodeMark",
};

/** Elements whose text is shown as it is written: code, and what later tickets render. */
const asWritten = new Set(["FencedCode", "CodeBlock", "HTMLBlock", "Frontmatter"]);

/**
 * What Live preview hides between `from` and `to`, in order: the marks of the elements the
 * selection is not in. A mark shows when its element touches a selection range and the mark is on
 * the line where that range ends, its head.
 */
export function hiddenRanges(state: EditorState, from = 0, to = state.doc.length): Span[] {
  return preview(state, from, to).hidden;
}

/**
 * The tables between `from` and `to` that Live preview shows as tables: those no selection range
 * touches. Each row stays one line, so the height does not change when the cursor enters or leaves.
 */
export function renderedTables(state: EditorState, from = 0, to = state.doc.length): RenderedTable[] {
  return preview(state, from, to).tables;
}

interface Preview {
  hidden: Span[];
  code: Span[];
  tables: RenderedTable[];
  /** The start of every table line, shown as a table or written: none wraps (tables.ts). */
  tableLines: number[];
}

/** The marks hidden, the inline code shown with a background and the tables, in one walk. */
function preview(state: EditorState, from: number, to: number, measuring = true): Preview {
  // A selection shows marks on the line where it ends only: dragging down, or ⌘A, leaves the
  // lines above as they were.
  const selections = state.selection.ranges.map((range) => ({
    from: range.from,
    to: range.to,
    line: state.doc.lineAt(range.head),
  }));
  const shows = (element: Span, mark: Span) =>
    selections.some(
      (range) =>
        element.from <= range.to && element.to >= range.from && mark.from >= range.line.from && mark.to <= range.line.to,
    );
  const hidden: Span[] = [];
  const code: Span[] = [];
  const tables: RenderedTable[] = [];
  const tableLines: number[] = [];
  // A table is written while a cursor or a selection's end is in it: ⌘A leaves tables shown.
  const entered = (table: Span) => state.selection.ranges.some((range) => range.head >= table.from && range.head <= table.to);
  const hide = (element: Span, mark: Span) => {
    if (mark.from < mark.to && !shows(element, mark)) hidden.push(mark);
  };

  syntaxTree(state).iterate({
    from,
    to,
    enter(ref) {
      const node = ref.node;
      if (asWritten.has(node.name)) return false;
      if (node.name === "Table") {
        for (let line = state.doc.lineAt(node.from); ; line = state.doc.line(line.number + 1)) {
          tableLines.push(line.from);
          if (line.to >= node.to) break;
        }
        // The cursor in a table shows it as written; inline elements in its cells follow their rules.
        if (entered(node)) return undefined;
        const table = tableLayout(state, node);
        tables.push(table);
        for (const row of table.rows) hidden.push(...rowHidden(state, row));
        return undefined;
      }
      if (node.name.startsWith("ATXHeading")) {
        for (const mark of children(node, "HeaderMark")) hide(node, headerMark(state, node, mark));
        return undefined;
      }
      const markName = emphasis[node.name];
      if (markName) {
        for (const mark of children(node, markName)) hide(node, mark);
        if (node.name !== "InlineCode") return undefined;
        // Inside inline code nothing is Markdown; inside the others, emphasis may nest.
        code.push({ from: node.from, to: node.to });
        return false;
      }
      switch (node.name) {
        case "Embed":
        case "Image": {
          // An image the app found is drawn under the line (embeds.ts): its source hides as a whole,
          // as one mark. A PDF Embed shows its name or Alias, a link that opens it, and so does a
          // missing file's, faded as an Unresolved link. The rest is as written.
          const file = embeddedFile(state, node);
          if (file?.kind === "image") hide(node, { from: node.from, to: node.to });
          else if (node.name === "Embed" && (file?.kind === "pdf" || isUnresolved(state, state.doc.sliceString(node.from + 3, node.to - 2)))) {
            for (const span of bracketed(state, node, 3)) hide(node, span);
          }
          return false;
        }
        case "Link": {
          const marks = children(node, "LinkMark");
          // `[text](url "title")`: the brackets, and from `]` to the end. A reference link
          // `[text][ref]`, and a link across lines (its title on the next), are left as written:
          // a line break cannot be hidden.
          const oneLine = state.doc.lineAt(node.from).number === state.doc.lineAt(node.to).number;
          if (marks.length >= 2 && node.getChild("URL") && oneLine) {
            hide(node, marks[0]);
            hide(node, { from: marks[1].from, to: node.to });
          }
          return undefined;
        }
        case "Autolink":
          for (const mark of children(node, "LinkMark")) hide(node, mark);
          return false;
        case "Wikilink": {
          for (const span of bracketed(state, node, 2)) hide(node, span);
          return false;
        }
        default:
          return undefined;
      }
    },
  });
  hidden.sort((a, b) => a.from - b.from);
  // What two rules hide (an escaped pipe in a Wikilink in a table) is hidden once.
  const merged: Span[] = [];
  for (const span of hidden) {
    const last = merged[merged.length - 1];
    if (last && span.from <= last.to) last.to = Math.max(last.to, span.to);
    else merged.push({ ...span });
  }
  // Over the whole table, not only the part in view: a column keeps its width while scrolling.
  if (measuring) for (const table of tables) measure(table, preview(state, table.from, table.to, false).hidden);
  return { hidden: merged, code, tables, tableLines };
}

/**
 * What a Wikilink or an Embed hides of itself (`open` is its opening's length, `[[` or `![[`): its
 * brackets, and with an Alias, `target|`. In a table the pipe is escaped, `target\\|alias`, and the
 * backslash goes too. An empty Alias shows the target.
 */
function bracketed(state: EditorState, node: SyntaxNode, open: number): Span[] {
  const inner = state.doc.sliceString(node.from + open, node.to - 2);
  const pipe = inner.indexOf("|");
  const start = node.from + open;
  if (pipe !== -1 && inner.slice(pipe + 1).trim() === "") {
    return [{ from: node.from, to: start }, { from: start + (inner[pipe - 1] === "\\" ? pipe - 1 : pipe), to: node.to }];
  }
  return [
    { from: node.from, to: pipe === -1 ? start : start + pipe + 1 },
    { from: node.to - 2, to: node.to },
  ];
}

function children(node: SyntaxNode, name: string): Span[] {
  return node.getChildren(name).map(({ from, to }) => ({ from, to }));
}

/** A Heading's `#`s with the spaces between them and its text, at the start or at the end. */
function headerMark(state: EditorState, heading: SyntaxNode, mark: Span): Span {
  const line = state.doc.sliceString(heading.from, heading.to);
  if (mark.from === heading.from) {
    let to = mark.to;
    while (to < heading.to && line[to - heading.from] === " ") to++;
    return { from: mark.from, to };
  }
  let from = mark.from;
  while (from > heading.from && line[from - 1 - heading.from] === " ") from--;
  return { from, to: mark.to };
}

const hide = Decoration.replace({});
const inlineCode = Decoration.mark({ class: "cm-inline-code" });

const tableLine = Decoration.line({ class: "cm-table-line" });
const rowLine = {
  header: Decoration.line({ class: "cm-table-line cm-table-row cm-table-header" }),
  rule: Decoration.line({ class: "cm-table-line cm-table-row cm-table-rule" }),
  body: Decoration.line({ class: "cm-table-line cm-table-row" }),
};

function decorations(view: EditorView): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  // A table across two visible ranges is drawn once.
  const drawn = new Set<number>();
  const shown = new Set<number>();
  for (const { from, to } of view.visibleRanges) {
    const { hidden, code, tables, tableLines } = preview(view.state, from, to);
    for (const span of hidden) ranges.push(hide.range(span.from, span.to));
    for (const span of code) ranges.push(inlineCode.range(span.from, span.to));
    for (const table of tables) {
      if (drawn.has(table.from)) continue;
      drawn.add(table.from);
      for (const row of table.rows) {
        shown.add(row.lineStart);
        ranges.push(rowLine[row.kind].range(row.lineStart));
        row.cells.forEach((cell, column) => {
          const { width, align } = table.columns[column];
          // In em, not ch: a bold header's ch is wider than a body's, and the columns must line up.
          const style = `width: calc(${width} * 0.6em + 1.4em); text-align: ${align}`;
          const empty = cell.text === "" ? " cm-table-empty" : "";
          ranges.push(Decoration.mark({ class: `cm-table-cell${empty}`, attributes: { style } }).range(cell.from, cell.to));
        });
      }
    }
    for (const line of tableLines) {
      if (!shown.has(line)) {
        shown.add(line);
        ranges.push(tableLine.range(line));
      }
    }
  }
  return Decoration.set(ranges, true);
}

export const livePreview = [
  ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) {
        this.decorations = decorations(view);
      }
      update(update: ViewUpdate) {
        const parsed = syntaxTree(update.startState) !== syntaxTree(update.state);
        if (update.docChanged || update.selectionSet || update.viewportChanged || parsed) {
          this.decorations = decorations(update.view);
        }
      }
    },
    { decorations: (plugin) => plugin.decorations },
  ),
  EditorView.baseTheme({
    ".cm-inline-code": { backgroundColor: "var(--code-background)", borderRadius: "3px", padding: "0 2px" },
    // A table's lines never wrap, shown as a table or written, so each keeps one line's height; a
    // wide one scrolls sideways instead.
    ".cm-table-line": { whiteSpace: "pre" },
    // The pipe an empty cell is drawn in: there, not seen, whatever colour the syntax gives it.
    ".cm-table-empty, .cm-table-empty *": { color: "transparent !important" },
    ".cm-table-cell": {
      display: "inline-block",
      boxSizing: "border-box",
      padding: "0 0.5em",
      overflow: "hidden",
      textOverflow: "ellipsis",
      whiteSpace: "pre",
      verticalAlign: "top",
      borderLeft: "1px solid var(--separator)",
    },
    ".cm-table-header": { fontWeight: "600", backgroundColor: "var(--code-background)" },
    ".cm-table-rule": { backgroundImage: "linear-gradient(var(--separator), var(--separator))", backgroundSize: "100% 1px", backgroundPosition: "center", backgroundRepeat: "no-repeat" },
  }),
];
