import { syntaxTree } from "@codemirror/language";
import { type EditorState, type Range, StateField } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, WidgetType } from "@codemirror/view";
import { styleTags, tags } from "@lezer/highlight";
import type { MarkdownConfig } from "@lezer/markdown";

// LaTeX, as Obsidian writes it: `$x^2$` in the text, `$$…$$` on its own, across lines too. Drawn
// by KaTeX while the cursor is out of it, written while the cursor is in it. KaTeX is a file of its
// own next to the page (katex.min.js, and its CSS with the fonts in it), loaded the first time a
// Note has a formula.

const dollar = 36; // $

/** A line that opens a block of LaTeX: `$$`, up to three spaces before it, as a fence. */
const blockStart = /^ {0,3}\$\$/;

/**
 * `$$` at a line's start opens a block of LaTeX of its own, as a fence opens code, closed by the
 * first line that ends with `$$` (the same line, for `$$…$$`): what is inside is not Markdown, so
 * a `=` on its own line is no Heading's underline. Never closed, it ends at a blank line and is
 * left as written. `$…$` in the text: the first `$` followed by no space, the last preceded by
 * none and followed by no digit, on one line and before any code, so `$5 and $10` is no formula.
 * `$$…$$` inside a paragraph too.
 */
export const math: MarkdownConfig = {
  defineNodes: [{ name: "BlockMath", block: true }, "InlineMath", "MathMark"],
  props: [styleTags({ "InlineMath BlockMath": tags.monospace, MathMark: tags.processingInstruction })],
  parseBlock: [
    {
      name: "BlockMath",
      before: "FencedCode",
      parse(cx, line) {
        if (!blockStart.test(line.text)) return false;
        const from = cx.lineStart;
        const open = from + line.text.indexOf("$$");
        const marks = [cx.elt("MathMark", open, open + 2)];
        let end = from + line.text.length;
        // `$$…$$` on one line, or the closing `$$` on a later one.
        let closed = /\$\$\s*$/.test(line.text.slice(line.text.indexOf("$$") + 2));
        while (!closed && cx.nextLine()) {
          if (line.text.trim() === "") break;
          end = cx.lineStart + line.text.length;
          closed = /\$\$\s*$/.test(line.text);
        }
        if (closed) {
          const close = end - (line.text.length - line.text.lastIndexOf("$$"));
          marks.push(cx.elt("MathMark", close, close + 2));
          cx.nextLine();
        }
        cx.addElement(cx.elt("BlockMath", from, end, marks));
        return true;
      },
      // A paragraph ends where a block of LaTeX starts, as it ends at a fence.
      endLeaf: (_cx, line) => blockStart.test(line.text),
    },
  ],
  parseInline: [
    {
      name: "Math",
      after: "InlineCode",
      parse(cx, next, pos) {
        if (next !== dollar) return -1;
        if (cx.char(pos + 1) === dollar) {
          const close = cx.slice(pos + 2, cx.end).indexOf("$$");
          if (close <= 0) return -1;
          const end = pos + 2 + close + 2;
          return cx.addElement(cx.elt("BlockMath", pos, end, [cx.elt("MathMark", pos, pos + 2), cx.elt("MathMark", end - 2, end)]));
        }
        if (/\s|^$/.test(cx.slice(pos + 1, pos + 2))) return -1;
        for (let at = pos + 1; at < cx.end; at++) {
          const char = cx.char(at);
          // Not across lines, nor into code: `$x$` there is code.
          if (char === 10 || char === 96) return -1;
          if (char === 92) {
            at++;
            continue;
          }
          if (char !== dollar) continue;
          if (/\s/.test(cx.slice(at - 1, at)) || /\d/.test(cx.slice(at + 1, at + 2))) return -1;
          return cx.addElement(cx.elt("InlineMath", pos, at + 1, [cx.elt("MathMark", pos, pos + 1), cx.elt("MathMark", at, at + 1)]));
        }
        return -1;
      },
    },
  ],
};

/** A formula: where it is written, its TeX without the dollars, and whether it is a block. */
export interface Formula {
  from: number;
  to: number;
  tex: string;
  display: boolean;
}

/** The Note's formulas, in order. */
export function formulas(state: EditorState): Formula[] {
  const found: Formula[] = [];
  syntaxTree(state).iterate({
    enter(node) {
      if (node.name !== "InlineMath" && node.name !== "BlockMath") return undefined;
      const marks = node.node.getChildren("MathMark");
      // A block never closed is left as written.
      if (marks.length < 2) return false;
      const [open, close] = [marks[0], marks[marks.length - 1]];
      found.push({ from: node.from, to: node.to, tex: state.sliceDoc(open.to, close.from).trim(), display: node.name === "BlockMath" });
      return false;
    },
  });
  return found;
}

export interface Katex {
  renderToString(tex: string, options: Record<string, unknown>): string;
}

let katex: Katex | null = null;
let loading: Promise<Katex> | null = null;

/** KaTeX and its stylesheet, loaded once from next to the page. */
export function loadKatex(): Promise<Katex> {
  loading ??= new Promise((resolve, reject) => {
    const style = document.createElement("link");
    style.rel = "stylesheet";
    style.href = "katex.min.css";
    document.head.append(style);
    const script = document.createElement("script");
    script.src = "katex.min.js";
    script.onload = () => {
      katex = (window as unknown as { katex: Katex }).katex;
      resolve(katex);
    };
    script.onerror = () => reject(new Error("katex.min.js is missing from the app"));
    document.head.append(script);
  });
  return loading;
}

class MathWidget extends WidgetType {
  constructor(
    readonly tex: string,
    readonly display: boolean,
    /** Where a click puts the cursor, to edit it: after its opening dollars. */
    readonly at: number,
  ) {
    super();
  }

  eq(other: MathWidget) {
    return other.tex === this.tex && other.display === this.display && other.at === this.at;
  }

  toDOM(view: EditorView) {
    const box = document.createElement("span");
    box.className = this.display ? "cm-math cm-math-display" : "cm-math";
    const draw = (library: Katex) => {
      // An error is drawn as KaTeX shows it, in red, not thrown.
      box.innerHTML = library.renderToString(this.tex, { displayMode: this.display, throwOnError: false, output: "html" });
    };
    if (katex) {
      draw(katex);
    } else {
      // The TeX as written until KaTeX arrives.
      box.textContent = this.tex;
      loadKatex().then(
        (library) => {
          draw(library);
          view.requestMeasure();
        },
        () => {},
      );
    }
    box.addEventListener("mousedown", (event) => {
      event.preventDefault();
      view.dispatch({ selection: { anchor: this.at } });
      view.focus();
    });
    return box;
  }

  ignoreEvent() {
    return false;
  }
}

function mathDecorations(state: EditorState): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  for (const formula of formulas(state)) {
    const entered = state.selection.ranges.some((range) => range.head >= formula.from && range.head <= formula.to);
    if (entered || formula.tex === "") continue;
    const widget = new MathWidget(formula.tex, formula.display, formula.from + (formula.display ? 2 : 1));
    ranges.push(Decoration.replace({ widget }).range(formula.from, formula.to));
  }
  return Decoration.set(ranges, true);
}

/** The formulas drawn: a `$$…$$` across lines is replaced across line breaks, so from the state. */
export const mathFormulas = [
  StateField.define<DecorationSet>({
    create: mathDecorations,
    update(value, transaction) {
      const changed = transaction.docChanged || transaction.selection || syntaxTree(transaction.startState) !== syntaxTree(transaction.state);
      return changed ? mathDecorations(transaction.state) : value;
    },
    provide: (field) => EditorView.decorations.from(field),
  }),
  EditorView.baseTheme({
    ".cm-math": { cursor: "text" },
    ".cm-math-display": { display: "block", textAlign: "center", padding: "0.4em 0" },
    ".cm-math-display .katex-display": { margin: "0" },
  }),
];
