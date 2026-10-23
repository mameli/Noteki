import { syntaxTree } from "@codemirror/language";
import type { EditorState, Range } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import type { SyntaxNode } from "@lezer/common";

// Live preview of inline syntax (SPEC, Live preview): bold, italic, strikethrough, inline code,
// links, Wikilinks and Headings show without their marks, and the marks of an element appear
// while the cursor is in it. Only on the cursor's line: what is shown above the cursor never
// changes as it moves, so lines above it never change height.

interface Span {
  from: number;
  to: number;
}

const emphasis: Record<string, string> = {
  Emphasis: "EmphasisMark",
  StrongEmphasis: "EmphasisMark",
  Strikethrough: "StrikethroughMark",
  InlineCode: "CodeMark",
};

/** Elements whose text is shown as it is written: code, and what later tickets render. */
const asWritten = new Set(["FencedCode", "CodeBlock", "Image", "Embed", "HTMLBlock", "Frontmatter"]);

/**
 * What Live preview hides between `from` and `to`, in order: the marks of the elements the
 * selection is not in. A mark shows when its element touches a selection range and the mark is on
 * the line where that range ends, its head.
 */
export function hiddenRanges(state: EditorState, from = 0, to = state.doc.length): Span[] {
  return preview(state, from, to).hidden;
}

/** The marks hidden, and the inline code shown with a background, in one walk of the tree. */
function preview(state: EditorState, from: number, to: number): { hidden: Span[]; code: Span[] } {
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
  const hide = (element: Span, mark: Span) => {
    if (mark.from < mark.to && !shows(element, mark)) hidden.push(mark);
  };

  syntaxTree(state).iterate({
    from,
    to,
    enter(ref) {
      const node = ref.node;
      if (asWritten.has(node.name)) return false;
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
          const marks = children(node, "WikilinkMark");
          if (marks.length !== 2) return false;
          // `[[target|alias]]` shows the alias: the target goes with the opening brackets. In a
          // table the pipe is escaped, `[[target\\|alias]]`, and the backslash goes too. An
          // empty Alias shows the target.
          const inner = state.doc.sliceString(marks[0].to, marks[1].from);
          const pipe = inner.indexOf("|");
          if (pipe !== -1 && inner.slice(pipe + 1).trim() === "") {
            hide(node, marks[0]);
            hide(node, { from: marks[0].to + (inner[pipe - 1] === "\\" ? pipe - 1 : pipe), to: marks[1].to });
          } else {
            hide(node, { from: marks[0].from, to: pipe === -1 ? marks[0].to : marks[0].to + pipe + 1 });
            hide(node, marks[1]);
          }
          return false;
        }
        default:
          return undefined;
      }
    },
  });
  return { hidden: hidden.sort((a, b) => a.from - b.from), code };
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

function decorations(view: EditorView): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  for (const { from, to } of view.visibleRanges) {
    const { hidden, code } = preview(view.state, from, to);
    for (const span of hidden) ranges.push(hide.range(span.from, span.to));
    for (const span of code) ranges.push(inlineCode.range(span.from, span.to));
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
  }),
];
