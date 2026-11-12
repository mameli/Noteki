import { syntaxTree } from "@codemirror/language";
import type { EditorState, Range } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import type { SyntaxNode } from "@lezer/common";
import { calloutOf, calloutTheme, drawCallout, quoteMarks, type RenderedCallout } from "./callouts";
import { embeddedFile, svgSource } from "./embeds";
import { drawFrontmatter, foldedLists, frontmatterLayout, frontmatterReading, frontmatterTheme, type RenderedFrontmatter } from "./frontmatter";
import { tagChipsTheme } from "./tagchips";
import { CheckboxWidget, isTicked, taskTheme } from "./tasks";
import { cursorInCells, fitColumns, measure, placeIn, type RenderedTable, rowHidden, tableKeys, tableLayout } from "./tables";
import { linesOf, type Span } from "./text";
import { isUnresolved } from "./wikilink";

export type { RenderedCallout } from "./callouts";
export type { RenderedFrontmatter } from "./frontmatter";
export type { RenderedTable } from "./tables";

// Live preview (SPEC, Live preview): bold, italic, strikethrough, highlights, inline code, links,
// Wikilinks and Headings show without their marks, and the marks of an element appear while the
// cursor is in it, only on the cursor's line; so do a task's bullet and brackets, drawn as a
// checkbox (tasks.ts), and a callout's `>` and `[!type]`, drawn as a box (callouts.ts). Tables are
// drawn as tables while the cursor is out of them (tables.ts), and code blocks in a box, their
// fences hidden while the cursor is out of them; Comments are hidden from the
// cursor's line up. What is shown above the cursor never changes as it moves, so lines above it
// never change height.

const emphasis: Record<string, string> = {
  Emphasis: "EmphasisMark",
  StrongEmphasis: "EmphasisMark",
  Strikethrough: "StrikethroughMark",
  Highlight: "HighlightMark",
  InlineCode: "CodeMark",
};

/** Elements whose text is shown as it is written: what later tickets render. */
const asWritten = new Set(["HTMLBlock"]);

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

/** The callouts between `from` and `to`, wherever the cursor is: only their marks show and hide. */
export function renderedCallouts(state: EditorState, from = 0, to = state.doc.length): RenderedCallout[] {
  return preview(state, from, to).callouts;
}

/**
 * The Frontmatter as Live preview draws it: its Properties table, or the line the app could not
 * read; null while it is written as it is (the cursor in it, no answer from the app yet) or none.
 */
export function renderedFrontmatter(state: EditorState): RenderedFrontmatter | null {
  return preview(state, 0, state.doc.length).frontmatter?.layout ?? null;
}

/** A code block, fenced or indented, drawn in a box: the start of each of its lines. */
export interface RenderedCode {
  lines: number[];
  /** The fence's language, `swift`, shown as a label. */
  info: Span | null;
}

/** The code blocks between `from` and `to`, wherever the cursor is: only their fences show and hide. */
export function renderedCode(state: EditorState, from = 0, to = state.doc.length): RenderedCode[] {
  return preview(state, from, to).codeBlocks;
}

/** A task's checkbox, drawn at `at`, the end of its hidden bullet and brackets. */
export interface RenderedTask {
  at: number;
  checked: boolean;
}

/**
 * The tasks between `from` and `to` shown with a checkbox: those whose bullet and brackets no
 * selection touches on the line where it ends.
 */
export function renderedTasks(state: EditorState, from = 0, to = state.doc.length): RenderedTask[] {
  return preview(state, from, to).tasks;
}

/**
 * The horizontal rules between `from` and `to` drawn as a line: the start of each one's line. A
 * rule's `---` shows, the line gone, while the cursor is on it.
 */
export function renderedRules(state: EditorState, from = 0, to = state.doc.length): number[] {
  return preview(state, from, to).rules;
}

interface Preview {
  hidden: Span[];
  rules: number[];
  code: Span[];
  codeBlocks: RenderedCode[];
  tables: RenderedTable[];
  tasks: RenderedTask[];
  callouts: RenderedCallout[];
  frontmatter: { node: SyntaxNode; layout: RenderedFrontmatter | null } | null;
  /** The text of done tasks, crossed out. */
  doneTasks: Span[];
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
  const codeBlocks: RenderedCode[] = [];
  const tables: RenderedTable[] = [];
  const tasks: RenderedTask[] = [];
  const doneTasks: Span[] = [];
  const callouts: RenderedCallout[] = [];
  const rules: number[] = [];
  let frontmatter: Preview["frontmatter"] = null;
  const tableLines: number[] = [];
  // A table is written while a cursor or a selection's end is in it: ⌘A leaves tables shown.
  const entered = (table: Span) => state.selection.ranges.some((range) => range.head >= table.from && range.head <= table.to);
  const hide = (element: Span, mark: Span) => {
    if (mark.from < mark.to && !shows(element, mark)) hidden.push(mark);
  };
  // A Comment shows from the line where a selection touching it ends, that line in the Comment,
  // down: the lines above the cursor keep their look.
  const showsComment = (comment: Span, first: number, line: number) =>
    selections.some(
      (range) => comment.from <= range.to && comment.to >= range.from && range.line.number >= first && range.line.number <= line,
    );

  syntaxTree(state).iterate({
    from,
    to,
    enter(ref) {
      const node = ref.node;
      // An SVG on one line is drawn under it (embeds.ts) and its source hides, as an image's. One
      // across lines stays as written above its drawing: its line breaks cannot hide.
      if (svgSource(state, node) !== null) {
        if (state.doc.lineAt(node.from).number === state.doc.lineAt(node.to).number) hide(node, { from: node.from, to: node.to });
        return false;
      }
      if (asWritten.has(node.name)) return false;
      if (node.name === "FencedCode" || node.name === "CodeBlock") {
        // The box stays; the fences hide while the cursor is out of the block, their lines kept
        // empty, so nothing moves up or down when it enters. Inside, nothing is Markdown.
        const info = node.getChild("CodeInfo");
        codeBlocks.push({ lines: Array.from(linesOf(state, node.from, node.to), (line) => line.from), info: info && { from: info.from, to: info.to } });
        if (!entered(node)) for (const mark of children(node, "CodeMark")) hidden.push(mark);
        return false;
      }
      if (node.name === "Table") {
        for (const line of linesOf(state, node.from, node.to)) tableLines.push(line.from);
        // Drawn with the cursor in it too, as in Obsidian: the cell it is in shows all its text
        // (tables.ts). Inline elements in the cells follow their rules.
        const table = tableLayout(state, node);
        for (const range of state.selection.ranges) {
          if (range.head < node.from || range.head > node.to) continue;
          const place = placeIn(state, table, range.head);
          if (place) (table.editing ??= []).push({ row: place.row, column: place.column });
        }
        tables.push(table);
        for (const row of table.rows) hidden.push(...rowHidden(state, row));
        return undefined;
      }
      if (node.name === "Frontmatter") {
        // The Properties table, drawn in place while the cursor is out of it (frontmatter.ts).
        const layout = frontmatterLayout(state, node, entered(node));
        frontmatter = { node, layout };
        if (layout) hidden.push(...layout.hidden);
        return false;
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
        case "HorizontalRule": {
          // Drawn as a line across the text, its line keeping its height (editor.css below).
          const rule = { from: node.from, to: node.to };
          if (shows(rule, rule)) return false;
          hidden.push(rule);
          rules.push(state.doc.lineAt(node.from).from);
          return false;
        }
        case "Autolink":
          for (const mark of children(node, "LinkMark")) hide(node, mark);
          return false;
        case "Wikilink": {
          for (const span of bracketed(state, node, 2)) hide(node, span);
          return false;
        }
        case "Blockquote": {
          // A callout's marks show line by line, as an inline element's: the box stays drawn.
          const callout = calloutOf(state, node);
          if (!callout) return undefined;
          for (const mark of quoteMarks(state, node)) hide(node, mark);
          const markerShown = shows(node, callout.marker);
          hide(node, callout.marker);
          const written = callout.title.from < callout.title.to;
          callouts.push({
            from: node.from,
            to: node.to,
            kind: callout.kind,
            title: callout.title,
            typeTitle: written || markerShown ? null : callout.typeTitle,
          });
          return undefined;
        }
        case "Task": {
          // A bullet goes with the brackets, as in Obsidian; a number stays.
          const marker = node.getChild("TaskMarker");
          if (!marker) return undefined;
          const checked = isTicked(state, marker);
          if (checked && marker.to < node.to) doneTasks.push({ from: marker.to, to: node.to });
          const bullet = node.parent?.getChild("ListMark");
          const bulleted = bullet && /^[-*+]$/.test(state.sliceDoc(bullet.from, bullet.to));
          const span = { from: bulleted ? bullet.from : marker.from, to: marker.to };
          if (!shows(span, span)) {
            hidden.push(span);
            tasks.push({ at: span.to, checked });
          }
          return undefined;
        }
        case "Comment":
        case "CommentBlock": {
          // Shown faded, as text to read: only its `%%` hide, while the cursor is out of it.
          const marks = node.getChildren("CommentMark");
          const comment = { from: node.from, to: marks.length > 1 ? marks[marks.length - 1].to : node.to };
          const first = state.doc.lineAt(comment.from).number;
          for (const mark of marks) {
            if (mark.from < from || mark.to > to) continue;
            if (!showsComment(comment, first, state.doc.lineAt(mark.from).number)) hidden.push({ from: mark.from, to: mark.to });
          }
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
  return { hidden: merged, rules, code, codeBlocks, tables, tasks, callouts, frontmatter, doneTasks, tableLines };
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
const doneTask = Decoration.mark({ class: "cm-task-done" });
const codeInfo = Decoration.mark({ class: "cm-code-info" });

const ruleLine = Decoration.line({ class: "cm-hr" });
const tableLine = Decoration.line({ class: "cm-table-line" });
const rowLine = {
  header: Decoration.line({ class: "cm-table-line cm-table-row cm-table-header" }),
  rule: Decoration.line({ class: "cm-table-line cm-table-row cm-table-rule" }),
  body: Decoration.line({ class: "cm-table-line cm-table-row" }),
};

/** A row with a cell being edited: as tall as that cell's text wrapped, the others as tall. */
const editedRowLine = {
  header: Decoration.line({ class: "cm-table-line cm-table-row cm-table-header cm-table-edited" }),
  rule: rowLine.rule,
  body: Decoration.line({ class: "cm-table-line cm-table-row cm-table-edited" }),
};

function decorations(view: EditorView): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  // A table across two visible ranges is drawn once.
  const drawn = new Set<number>();
  const shown = new Set<number>();
  const calloutLines = new Set<number>();
  const codeLines = new Set<number>();
  let frontmatterDrawn = false;
  const available = textWidth(view);
  for (const { from, to } of view.visibleRanges) {
    const { hidden, rules, code, codeBlocks, tables, tasks, callouts, frontmatter, doneTasks, tableLines } = preview(view.state, from, to);
    if (frontmatter && !frontmatterDrawn) {
      frontmatterDrawn = true;
      ranges.push(...drawFrontmatter(view.state, frontmatter.node, frontmatter.layout));
    }
    for (const span of hidden) ranges.push(hide.range(span.from, span.to));
    for (const line of rules) ranges.push(ruleLine.range(line));
    for (const span of code) ranges.push(inlineCode.range(span.from, span.to));
    for (const task of tasks) ranges.push(Decoration.widget({ widget: new CheckboxWidget(task.checked), side: 1 }).range(task.at));
    for (const span of doneTasks) ranges.push(doneTask.range(span.from, span.to));
    for (const callout of callouts) ranges.push(...drawCallout(view.state, callout, from, to, calloutLines));
    for (const block of codeBlocks) {
      if (block.info && !codeLines.has(block.lines[0])) ranges.push(codeInfo.range(block.info.from, block.info.to));
      block.lines.forEach((line, index) => {
        if (line < from || line > to || codeLines.has(line)) return;
        codeLines.add(line);
        const edges = `${index === 0 ? " cm-code-first" : ""}${index === block.lines.length - 1 ? " cm-code-last" : ""}`;
        ranges.push(Decoration.line({ class: `cm-code-block${edges}` }).range(line));
      });
    }
    for (const table of tables) {
      if (drawn.has(table.from)) continue;
      drawn.add(table.from);
      const lastRow = table.rows[table.rows.length - 1];
      // Never wider than the text: past it, the widest columns shrink, cut with an ellipsis
      // (fitColumns). As a share of the line too, should the text be narrower than measured.
      const natural = table.columns.map((column) => column.width * 0.6 + 1.4);
      const widths = fitColumns(natural, available - 0.2);
      const total = widths.reduce((sum, width) => sum + width, 0);
      const editing = table.editing ?? [];
      table.rows.forEach((row, index) => {
        shown.add(row.lineStart);
        const edited = editing.some((place) => place.row === index);
        ranges.push((edited ? editedRowLine : rowLine)[row.kind].range(row.lineStart));
        row.cells.forEach((cell, column) => {
          const { align } = table.columns[column];
          // In em, not ch: a bold header's ch is wider than a body's, and the columns must line up.
          const share = (widths[column] / total) * 100;
          const style = `width: min(${widths[column].toFixed(3)}em, ${share.toFixed(3)}%); text-align: ${align}`;
          let classes = "cm-table-cell";
          if (row.kind === "header") classes += " cm-table-head";
          if (row === lastRow) classes += " cm-table-bottom";
          if (column === 0) classes += " cm-table-first";
          if (column === table.columns.length - 1) classes += " cm-table-last";
          if (cell.text === "") classes += " cm-table-empty";
          if (editing.some((place) => place.row === index && place.column === column)) classes += " cm-table-editing";
          // Inclusive, so it wraps what starts and ends with it, a cell of nothing but `code`: inside
          // the code's span, the cell would be pushed sideways by the code's padding, and the
          // table's sides would not be straight.
          ranges.push(Decoration.mark({ class: classes, attributes: { style }, inclusive: true }).range(cell.from, cell.to));
        });
      });
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

/** The width of the text, in em of its font, for the tables to fit in; 0 before it is laid out. */
function textWidth(view: EditorView): number {
  const content = view.contentDOM;
  const style = getComputedStyle(content);
  const fontSize = parseFloat(style.fontSize);
  const width = content.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
  return fontSize > 0 && width > 0 ? width / fontSize : 0;
}

export const livePreview = [
  tableKeys,
  cursorInCells,
  ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) {
        this.decorations = decorations(view);
      }
      update(update: ViewUpdate) {
        const parsed = syntaxTree(update.startState) !== syntaxTree(update.state);
        // The app's answer about the Frontmatter draws the Properties table.
        const answered = update.startState.field(frontmatterReading, false) !== update.state.field(frontmatterReading, false);
        // The window resized: the tables fit its new width.
        if (update.docChanged || update.selectionSet || update.viewportChanged || update.geometryChanged || parsed || answered) {
          this.decorations = decorations(update.view);
        }
      }
    },
    { decorations: (plugin) => plugin.decorations },
  ),
  taskTheme,
  calloutTheme,
  foldedLists,
  frontmatterTheme,
  tagChipsTheme,
  EditorView.baseTheme({
    // A rule in the middle of its line, which keeps one line's height.
    ".cm-hr": {
      backgroundImage: "linear-gradient(var(--separator), var(--separator))",
      backgroundSize: "100% 1px",
      backgroundPosition: "center",
      backgroundRepeat: "no-repeat",
      backgroundClip: "content-box",
    },
    ".cm-inline-code": { backgroundColor: "var(--code-background)", borderRadius: "3px", padding: "0 2px" },
    ".cm-code-block": { backgroundColor: "var(--code-background)", padding: "0 1em", fontFamily: "var(--mono)", fontSize: "0.92em" },
    ".cm-code-first": { borderTopLeftRadius: "6px", borderTopRightRadius: "6px" },
    ".cm-code-last": { borderBottomLeftRadius: "6px", borderBottomRightRadius: "6px" },
    ".cm-code-info": { color: "var(--syntax)", fontFamily: "var(--system-font)", fontSize: "0.85em" },
    // A table's rows drawn never wrap, so each keeps one line's height: its columns shrink to fit
    // the text's width instead. Written, while the cursor is in it, a row longer than the text
    // wraps: nothing scrolls sideways.
    ".cm-table-row": { whiteSpace: "pre" },
    // The pipe an empty cell is drawn in: there, not seen, whatever colour the syntax gives it.
    ".cm-table-empty, .cm-table-empty *": { color: "transparent !important" },
    ".cm-table-cell": {
      display: "inline-block",
      boxSizing: "border-box",
      // One line's height, borders included: a row as tall shown as written.
      height: "1.6em",
      padding: "0 0.5em",
      overflow: "hidden",
      textOverflow: "ellipsis",
      whiteSpace: "pre",
      verticalAlign: "top",
      borderRight: "1px solid var(--separator)",
      borderBottom: "1px solid var(--separator)",
    },
    ".cm-table-first": { borderLeft: "1px solid var(--separator)" },
    // The row of the cell being edited grows with its text, wrapped in its column, and its other
    // cells grow with it: a flex line. Only the cursor's line changes height, as anywhere.
    ".cm-table-edited": { display: "flex", alignItems: "stretch" },
    ".cm-table-edited .cm-table-cell": { flex: "none", height: "auto", minHeight: "1.6em" },
    ".cm-table-edited .cm-table-head": { minHeight: "3.2em" },
    ".cm-table-editing": {
      whiteSpace: "pre-wrap",
      overflowWrap: "anywhere",
      textOverflow: "clip",
      boxShadow: "inset 0 0 0 2px rgba(var(--accent-rgb), 0.45)",
    },
    ".cm-table-editing.cm-table-empty, .cm-table-editing.cm-table-empty *": { color: "inherit !important" },
    // A header's cells reach down over the delimiter row, whose text is all hidden, their text in
    // the middle: one row twice as tall, while each line keeps one line's height (its negative
    // margin), so nothing moves when the cursor enters the table.
    ".cm-table-head": {
      fontWeight: "600",
      backgroundColor: "var(--code-background)",
      borderTop: "1px solid var(--separator)",
      height: "3.2em",
      paddingTop: "0.8em",
      marginBottom: "-1.6em",
    },
    ".cm-table-head.cm-table-first": { borderTopLeftRadius: "6px" },
    ".cm-table-head.cm-table-last": { borderTopRightRadius: "6px" },
    ".cm-table-bottom.cm-table-first": { borderBottomLeftRadius: "6px" },
    ".cm-table-bottom.cm-table-last": { borderBottomRightRadius: "6px" },
  }),
];
