import { syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import { EditorView, WidgetType } from "@codemirror/view";
import type { Tree } from "@lezer/common";
import { styleTags, tags } from "@lezer/highlight";
import type { BlockContext, Element, Line, MarkdownConfig } from "@lezer/markdown";

// Footnotes, as Obsidian and GitHub write them: `[^label]` in the text, and its definition
// `[^label]: text` on a line of its own, continued by the lines indented under it. Live preview
// shows a reference as a small number, in the order the references first appear, and a
// definition's `[^label]:` as that number; a click on one goes to the other.

const bracket = 91; // [
const caret = 94; // ^
const closing = 93; // ]
const definitionStart = /^\[\^([^\]\s]+)\]:[ \t]?/;

function definitionAt(line: Line): RegExpExecArray | null {
  return line.indent >= 4 ? null : definitionStart.exec(line.text.slice(line.pos));
}

/** The inline elements of `text`, which starts at `offset` in the Note. */
function inline(cx: BlockContext, text: string, offset: number): Element[] {
  return cx.parser.parseInline(text, offset);
}

export const footnotes: MarkdownConfig = {
  defineNodes: [{ name: "FootnoteDefinition", block: true }, "FootnoteReference", "FootnoteMark", "FootnoteLabel"],
  props: [
    styleTags({
      FootnoteReference: tags.link,
      "FootnoteMark FootnoteLabel": tags.processingInstruction,
    }),
  ],
  parseBlock: [
    {
      name: "FootnoteDefinition",
      before: "IndentedCode",
      // Right under a paragraph's line too, ending it.
      endLeaf: (_cx, line) => definitionAt(line) !== null,
      parse(cx, line) {
        const found = definitionAt(line);
        if (!found) return false;
        const from = cx.lineStart + line.pos;
        const labelEnd = from + 2 + found[1].length;
        const children: Element[] = [
          cx.elt("FootnoteMark", from, from + 2),
          cx.elt("FootnoteLabel", from + 2, labelEnd),
          cx.elt("FootnoteMark", labelEnd, labelEnd + 2),
        ];
        const contentStart = line.pos + found[0].length;
        children.push(...inline(cx, line.text.slice(contentStart), cx.lineStart + contentStart));
        let end = cx.lineStart + line.text.length;
        // The lines indented under it continue it; a blank line or one at the margin ends it.
        while (cx.nextLine()) {
          if (line.pos === line.text.length || line.indent < 2 || definitionAt(line)) break;
          children.push(...inline(cx, line.text.slice(line.pos), cx.lineStart + line.pos));
          end = cx.lineStart + line.text.length;
        }
        cx.addElement(cx.elt("FootnoteDefinition", from, end, children));
        return true;
      },
    },
  ],
  parseInline: [
    {
      name: "FootnoteReference",
      before: "Link",
      parse(cx, next, pos) {
        if (next !== bracket || cx.char(pos + 1) !== caret) return -1;
        let close = pos + 2;
        while (close < cx.end) {
          const char = cx.char(close);
          if (char === closing) break;
          if (char === bracket || char === 32 || char === 9 || char === 10) return -1;
          close++;
        }
        if (close === pos + 2 || close >= cx.end) return -1;
        return cx.addElement(
          cx.elt("FootnoteReference", pos, close + 1, [
            cx.elt("FootnoteMark", pos, pos + 2),
            cx.elt("FootnoteLabel", pos + 2, close),
            cx.elt("FootnoteMark", close, close + 1),
          ]),
        );
      },
    },
  ],
};

/** The label of a footnote node, a reference or a definition: what is between `[^` and `]`. */
export function footnoteLabel(state: EditorState, from: number): string {
  const text = state.doc.sliceString(from + 2, Math.min(state.doc.length, from + 200));
  return text.slice(0, text.indexOf("]"));
}

const numbering = new WeakMap<Tree, Map<string, number>>();

/**
 * The number of each label, from 1, in the order its references first appear in the Note (as
 * far as it is parsed); a definition without a reference has none.
 */
export function footnoteNumbers(state: EditorState): Map<string, number> {
  const tree = syntaxTree(state);
  let numbers = numbering.get(tree);
  if (numbers) return numbers;
  numbers = new Map();
  const found = numbers;
  tree.iterate({
    enter(node) {
      if (node.name !== "FootnoteReference") return undefined;
      const label = footnoteLabel(state, node.from);
      if (!found.has(label)) found.set(label, found.size + 1);
      return false;
    },
  });
  numbering.set(tree, found);
  return found;
}

/** Where the definition of `label` starts, or where its first reference is; null if none. */
function find(state: EditorState, name: "FootnoteDefinition" | "FootnoteReference", label: string): number | null {
  let at: number | null = null;
  syntaxTree(state).iterate({
    enter(node) {
      if (at !== null) return false;
      if (node.name === name && footnoteLabel(state, node.from) === label) {
        at = node.from;
        return false;
      }
      return undefined;
    },
  });
  return at;
}

/** A reference drawn as its number, or a definition's mark as its number and a dot; a click goes to the other. */
export class FootnoteWidget extends WidgetType {
  constructor(
    readonly label: string,
    readonly number: number | undefined,
    readonly definition: boolean,
  ) {
    super();
  }

  eq(other: FootnoteWidget) {
    return other.label === this.label && other.number === this.number && other.definition === this.definition;
  }

  toDOM(view: EditorView) {
    const mark = document.createElement("span");
    mark.className = this.definition ? "cm-footnote-label" : "cm-footnote-ref";
    const shown = this.number === undefined ? this.label : String(this.number);
    mark.textContent = this.definition ? `${shown}.` : shown;
    mark.title = this.definition ? "Go to the reference" : "Go to the footnote";
    mark.addEventListener("mousedown", (event) => {
      event.preventDefault();
      const target = find(view.state, this.definition ? "FootnoteReference" : "FootnoteDefinition", this.label);
      if (target === null) return;
      // On the text after the mark: the cursor there shows the definition as written.
      const anchor = this.definition ? target : Math.min(view.state.doc.length, target + this.label.length + 4);
      view.dispatch({ selection: { anchor }, effects: EditorView.scrollIntoView(anchor, { y: "center" }), userEvent: "select" });
      view.focus();
    });
    return mark;
  }

  ignoreEvent() {
    return true;
  }
}

export const footnoteTheme = EditorView.baseTheme({
  // Raised without changing the line's height: a line keeps it whether the cursor is in it or not.
  ".cm-footnote-ref": {
    position: "relative",
    top: "-0.45em",
    fontSize: "0.72em",
    color: "var(--link)",
    cursor: "pointer",
    padding: "0 1px",
  },
  ".cm-footnote-label": { color: "var(--syntax)", cursor: "pointer", marginRight: "0.4em" },
});
