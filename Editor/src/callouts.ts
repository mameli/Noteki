import { syntaxTree } from "@codemirror/language";
import type { EditorState, Range } from "@codemirror/state";
import { Decoration, EditorView, WidgetType } from "@codemirror/view";
import type { SyntaxNode } from "@lezer/common";
import { linesOf, type Span } from "./text";

// Callouts, `> [!note] Title`, as Obsidian draws them: a box in the colour of their type, with its
// title, always open (`[!tip]-` folds in Obsidian, not here). Only a quote at the top level is a
// callout: one inside another quote is left as written.

/** Obsidian's types, by their names and aliases; any other type is a note. */
const kinds: Record<string, CalloutKind> = {
  note: "note",
  abstract: "abstract", summary: "abstract", tldr: "abstract",
  info: "info",
  todo: "todo",
  tip: "tip", hint: "tip", important: "tip",
  success: "success", check: "success", done: "success",
  question: "question", help: "question", faq: "question",
  warning: "warning", caution: "warning", attention: "warning",
  failure: "failure", fail: "failure", missing: "failure",
  danger: "danger", error: "danger",
  bug: "bug",
  example: "example",
  quote: "quote", cite: "quote",
};

/** Each type's colour, as Obsidian's. */
const colours = {
  note: "8, 109, 221",
  abstract: "0, 191, 188",
  info: "8, 109, 221",
  todo: "8, 109, 221",
  tip: "0, 191, 188",
  success: "8, 185, 78",
  question: "236, 117, 0",
  warning: "236, 117, 0",
  failure: "233, 49, 71",
  danger: "233, 49, 71",
  bug: "233, 49, 71",
  example: "120, 82, 238",
  quote: "158, 158, 158",
};

export type CalloutKind = keyof typeof colours;

/** `> [!type]± title`: what follows the first line's `>`. */
const header = /^(\s*)\[!([\w-]+)\][+-]?\s*(.*)$/;

export interface Callout {
  kind: CalloutKind;
  /** `[!type]`, its fold sign and the spaces after. */
  marker: Span;
  /** Its title as written, after the marker: maybe empty. */
  title: Span;
  /** The type with a capital: the title Obsidian shows when none is written. */
  typeTitle: string;
}

/** The callout a quote is, or null. */
export function calloutOf(state: EditorState, quote: SyntaxNode): Callout | null {
  for (let parent = quote.parent; parent; parent = parent.parent) if (parent.name === "Blockquote") return null;
  const mark = quote.firstChild;
  if (mark?.name !== "QuoteMark") return null;
  const line = state.doc.lineAt(quote.from);
  const found = header.exec(line.text.slice(mark.to - line.from));
  if (!found) return null;
  const [all, spaces, type, written] = found;
  const markerEnd = mark.to + all.length - written.length;
  return {
    kind: kinds[type.toLowerCase()] ?? "note",
    marker: { from: mark.to + spaces.length, to: markerEnd },
    title: { from: markerEnd, to: line.to },
    typeTitle: type[0].toUpperCase() + type.slice(1),
  };
}

/**
 * The `>` of each line of a callout, with the space after it: the first on its line, a list's
 * bullet or indent before it, a quote inside the callout after it.
 */
export function quoteMarks(state: EditorState, quote: SyntaxNode): Span[] {
  const marks: Span[] = [];
  let lastLine = -1;
  syntaxTree(state).iterate({
    from: quote.from,
    to: quote.to,
    enter(node) {
      if (node.name !== "QuoteMark" || node.from < quote.from) return;
      const line = state.doc.lineAt(node.from).number;
      if (line === lastLine) return;
      lastLine = line;
      marks.push({ from: node.from, to: state.sliceDoc(node.to, node.to + 1) === " " ? node.to + 1 : node.to });
    },
  });
  return marks;
}

/** A callout, drawn as a box over its lines. */
export interface RenderedCallout extends Span {
  kind: CalloutKind;
  /** Its written title, drawn as a title. */
  title: Span;
  /** Drawn in place of its hidden `[!type]` when no title is written: its type; null otherwise. */
  typeTitle: string | null;
}

/** Its type, drawn as its title when none is written. */
class TypeTitleWidget extends WidgetType {
  constructor(readonly title: string) {
    super();
  }

  eq(other: TypeTitleWidget) {
    return other.title === this.title;
  }

  toDOM() {
    const title = document.createElement("span");
    title.className = "cm-callout-title";
    title.textContent = this.title;
    return title;
  }
}

const titleMark = Decoration.mark({ class: "cm-callout-title" });

/**
 * A callout's box on its lines between `from` and `to`, but those in `drawn` (a callout across two
 * visible ranges is drawn once), and its title.
 */
export function drawCallout(state: EditorState, callout: RenderedCallout, from: number, to: number, drawn: Set<number>): Range<Decoration>[] {
  const ranges: Range<Decoration>[] = [];
  const first = state.doc.lineAt(callout.from).number;
  const last = state.doc.lineAt(callout.to).number;
  for (const line of linesOf(state, Math.max(callout.from, from), Math.min(callout.to, to))) {
    if (drawn.has(line.from)) continue;
    drawn.add(line.from);
    const edges = (line.number === first ? " cm-callout-first" : "") + (line.number === last ? " cm-callout-last" : "");
    ranges.push(Decoration.line({ class: `cm-callout cm-callout-${callout.kind}${edges}` }).range(line.from));
  }
  if (callout.title.from < callout.title.to) ranges.push(titleMark.range(callout.title.from, callout.title.to));
  if (callout.typeTitle !== null) {
    ranges.push(Decoration.widget({ widget: new TypeTitleWidget(callout.typeTitle), side: 1 }).range(callout.title.to));
  }
  return ranges;
}

export const calloutTheme = EditorView.baseTheme({
  ...Object.fromEntries(Object.entries(colours).map(([kind, rgb]) => [`.cm-callout-${kind}`, { "--callout": rgb }])),
  ".cm-callout": {
    "--quote-text": "var(--text)",
    backgroundColor: "rgba(var(--callout), 0.1)",
    borderLeft: "3px solid rgb(var(--callout))",
    paddingLeft: "12px",
  },
  ".cm-callout-first": { paddingTop: "4px", borderTopRightRadius: "4px" },
  ".cm-callout-last": { paddingBottom: "4px", borderBottomRightRadius: "4px" },
  ".cm-callout-title": { "--quote-text": "rgb(var(--callout))", color: "rgb(var(--callout))", fontWeight: "600" },
});
