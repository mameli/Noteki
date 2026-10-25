import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import { type EditorState, type Range, StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView, WidgetType } from "@codemirror/view";
import { styleTags, tags } from "@lezer/highlight";
import type { MarkdownConfig } from "@lezer/markdown";
import type { SyntaxNode } from "@lezer/common";
import { AddTagWidget, TagChipWidget } from "./tagchips";
import { linesOf, type Span } from "./text";

// The YAML block at the very top of a Note, between `---` lines, as Obsidian reads it.
// Without this, `tags: x` followed by `---` parses as a Markdown heading. The parser sees one
// line at a time and cannot go back: a Note that opens with `---` and never closes it is all
// Frontmatter, a rare case accepted for now.
const fence = /^---[ \t]*$/;
const closing = /^(---|\.\.\.)[ \t]*$/;

export const frontmatter: MarkdownConfig = {
  defineNodes: [{ name: "Frontmatter", block: true }],
  props: [styleTags({ Frontmatter: tags.meta })],
  parseBlock: [
    {
      name: "Frontmatter",
      before: "HorizontalRule",
      parse(cx, line) {
        if (cx.lineStart !== 0 || !fence.test(line.text)) return false;
        let end = line.text.length;
        while (cx.nextLine()) {
          end = cx.lineStart + line.text.length;
          if (closing.test(line.text)) {
            cx.nextLine();
            break;
          }
        }
        cx.addElement(cx.elt("Frontmatter", 0, end));
        return true;
      },
    },
  ],
};

// The Properties table (SPEC, Frontmatter). The app reads the YAML (NotekiCore's Frontmatter) and
// says where each key and value is; the editor draws the table in place, line by line, as it
// draws tables (ADR 0008): every line stays one line, never wrapped, so the text moves sideways
// and never up or down when the cursor enters the YAML or leaves it.

/** A part of one line: `line` from 1, the columns in the line. */
export interface LineSpan {
  line: number;
  from: number;
  to: number;
}

export type PropertyValue = { kind: "none" } | { kind: "text"; span: LineSpan } | { kind: "list"; items: LineSpan[] };

export interface Property {
  key: string;
  keySpan: LineSpan;
  value: PropertyValue;
}

/** The app's reading: the Properties, or the line where it could not read the YAML. */
export type FrontmatterReading = { properties: Property[] } | { unreadable: number };

/** The app's answer about the Frontmatter `text`; null when it never closes. */
export interface FrontmatterAnswer {
  text: string;
  reading: FrontmatterReading | null;
}

export const setFrontmatter = StateEffect.define<FrontmatterAnswer>();

export const frontmatterReading = StateField.define<FrontmatterAnswer | null>({
  create: () => null,
  update(value, transaction) {
    for (const effect of transaction.effects) if (effect.is(setFrontmatter)) value = effect.value;
    return value;
  },
});

function frontmatterNode(state: EditorState): SyntaxNode | null {
  const node = syntaxTree(state).topNode.firstChild;
  return node?.name === "Frontmatter" ? node : null;
}

/** The Frontmatter, from its first `---` line to its last, to ask the app about; null if none or never closed. */
export function frontmatterText(state: EditorState): string | null {
  const node = frontmatterNode(state);
  if (!node || node.from === node.to) return null;
  const last = state.doc.lineAt(node.to);
  if (last.number === 1 || !closing.test(last.text)) return null;
  return state.sliceDoc(node.from, node.to);
}

/** Where the Note's text starts: after the Frontmatter, where the cursor goes on opening. */
export function bodyStart(state: EditorState): number {
  ensureSyntaxTree(state, Math.min(state.doc.length, 100_000), 50);
  const node = frontmatterNode(state);
  if (!node || frontmatterText(state) === null) return 0;
  // A Note of nothing but its Frontmatter has no text to start: it opens on the YAML.
  const last = state.doc.lineAt(node.to);
  return last.number < state.doc.lines ? last.to + 1 : last.to;
}

/** The Frontmatter as Live preview draws it. */
export interface RenderedFrontmatter extends Span {
  /** The widest key, in characters: the width of the keys' column. */
  keyWidth: number;
  keys: Span[];
  /** Text values, as written. */
  values: Span[];
  /** The lists' items, one per line or side by side. */
  items: Span[];
  /** The `tags` Property's chips, and where the field to add one is. */
  tags: { chips: TagChip[]; addAt: number };
  /** What hides: the `---` lines, the colons, quotes, brackets, commas, dashes and comments. */
  hidden: Span[];
  /** The start of the lines of items under their key. */
  itemLines: number[];
  /** When the app could not read the YAML, the line where it stopped; drawn as written. */
  unreadableLine?: number;
}

/**
 * How the Frontmatter `node` is drawn when the cursor is out of it: null while the app has not
 * answered about it as it is now, when it is not read, or while it is entered (`entered`).
 */
export function frontmatterLayout(state: EditorState, node: SyntaxNode, entered: boolean): RenderedFrontmatter | null {
  const answer = state.field(frontmatterReading, false);
  if (!answer?.reading || answer.text !== state.sliceDoc(node.from, node.to)) return null;
  // Without a `tags` Property, a Tag is added from the table's top.
  const tags = { chips: [], addAt: state.doc.lineAt(node.from).to };
  const empty = { from: node.from, to: node.to, keyWidth: 0, keys: [], values: [], items: [], tags, hidden: [], itemLines: [] };
  if ("unreadable" in answer.reading) return { ...empty, unreadableLine: answer.reading.unreadable };
  if (entered) return null;
  const at = (span: LineSpan) => {
    const line = state.doc.line(span.line);
    return { from: line.from + span.from, to: line.from + span.to };
  };
  const layout: RenderedFrontmatter = empty;
  const first = state.doc.lineAt(node.from);
  const last = state.doc.lineAt(node.to);
  layout.hidden.push({ from: first.from, to: first.to }, { from: last.from, to: last.to });
  for (const property of answer.reading.properties) {
    const key = at(property.keySpan);
    layout.keys.push(key);
    layout.keyWidth = Math.max(layout.keyWidth, property.key.length);
    const keyLine = state.doc.line(property.keySpan.line);
    const isTags = property.key === "tags";
    // The Tags are chips: a text of several, `a, b`, a chip each, as NotekiCore reads it.
    const values: LineSpan[] = property.value.kind === "text" ? [property.value.span] : property.value.kind === "list" ? property.value.items : [];
    const cells = isTags && property.value.kind === "text" ? values.flatMap((cell) => tagsIn(state, cell)) : values;
    // What shows on each line, in order: what is between and after them hides.
    const shown: Span[] = [key];
    let lastLine = keyLine;
    for (const cell of cells) {
      const span = at(cell);
      if (isTags) layout.tags.chips.push({ ...span, name: state.sliceDoc(span.from, span.to).replace(/^#/, "") });
      else if (property.value.kind === "text") layout.values.push(span);
      else layout.items.push(span);
      if (cell.line === property.keySpan.line) {
        shown.push(span);
      } else {
        lastLine = state.doc.line(cell.line);
        layout.itemLines.push(lastLine.from);
        hideAround(layout.hidden, lastLine, [span]);
      }
    }
    hideAround(layout.hidden, keyLine, shown);
    if (isTags) layout.tags.addAt = lastLine.to;
  }
  // A comment on a line of its own: an empty row. The app reads past it, so it says nothing of it.
  for (const line of linesOf(state, first.to + 1, last.from - 1)) {
    if (/^\s*#/.test(line.text)) layout.hidden.push({ from: line.from, to: line.to });
  }
  layout.hidden.sort((a, b) => a.from - b.from);
  return layout;
}

/** A Tag of the `tags` Property, drawn as a chip: `name` without its `#`. */
export interface TagChip extends Span {
  name: string;
}

/** The Tags in a text value, `a, b` or `a b`, each where it is written. */
function tagsIn(state: EditorState, cell: LineSpan): LineSpan[] {
  const text = state.doc.line(cell.line).text.slice(cell.from, cell.to);
  return [...text.matchAll(/[^\s,]+/g)].map((found) => ({ line: cell.line, from: cell.from + found.index, to: cell.from + found.index + found[0].length }));
}

/** Hides what `line` holds between and after the spans shown on it, and before the first. */
function hideAround(hidden: Span[], line: { from: number; to: number }, shown: Span[]) {
  let at = line.from;
  for (const span of shown) {
    if (span.from > at) hidden.push({ from: at, to: span.from });
    at = Math.max(at, span.to);
  }
  if (line.to > at) hidden.push({ from: at, to: line.to });
}

class LabelWidget extends WidgetType {
  eq() {
    return true;
  }

  toDOM() {
    const label = document.createElement("span");
    label.className = "cm-properties-label";
    label.textContent = "Properties";
    return label;
  }
}

const keyMark = Decoration.mark({ class: "cm-property-key" });
const valueMark = Decoration.mark({ class: "cm-property-value" });
const itemMark = Decoration.mark({ class: "cm-property-item" });
const unreadableLine = Decoration.line({ class: "cm-frontmatter-unreadable", attributes: { title: "Noteki cannot read this YAML: it is left as written." } });
const label = Decoration.widget({ widget: new LabelWidget(), side: 1 });

/**
 * The Frontmatter's lines, never wrapped whether drawn or written, and the table's marks. The
 * keys' width goes on every line: the items under a key start where the values do.
 */
export function drawFrontmatter(state: EditorState, node: SyntaxNode, layout: RenderedFrontmatter | null): Range<Decoration>[] {
  const ranges: Range<Decoration>[] = [];
  const style = `--key-width: calc(${layout?.keyWidth ?? 0} * 0.6em + 1em)`;
  const drawn = layout !== null && layout.unreadableLine === undefined;
  const first = state.doc.lineAt(node.from).number;
  const last = state.doc.lineAt(node.to).number;
  for (const line of linesOf(state, node.from, node.to)) {
    let className = "cm-frontmatter-line";
    if (drawn) {
      className += " cm-properties";
      if (line.number === first) className += " cm-properties-first";
      if (line.number === last) className += " cm-properties-last";
      if (layout.itemLines.includes(line.from)) className += " cm-property-items";
    }
    ranges.push(Decoration.line({ class: className, attributes: { style } }).range(line.from));
    if (layout?.unreadableLine === line.number) ranges.push(unreadableLine.range(line.from));
  }
  if (!drawn) return ranges;
  ranges.push(label.range(state.doc.lineAt(node.from).to));
  for (const chip of layout.tags.chips) ranges.push(Decoration.replace({ widget: new TagChipWidget(chip.name) }).range(chip.from, chip.to));
  const present = layout.tags.chips.map((chip) => chip.name);
  ranges.push(Decoration.widget({ widget: new AddTagWidget(present), side: 2 }).range(layout.tags.addAt));
  for (const span of layout.keys) ranges.push(keyMark.range(span.from, span.to));
  for (const span of layout.values) if (span.from < span.to) ranges.push(valueMark.range(span.from, span.to));
  for (const span of layout.items) if (span.from < span.to) ranges.push(itemMark.range(span.from, span.to));
  return ranges;
}

export const frontmatterTheme = EditorView.baseTheme({
  // Never wrapped, drawn or written: each line keeps one line's height (ADR 0008).
  ".cm-frontmatter-line": { whiteSpace: "pre" },
  ".cm-properties": { "--frontmatter-text": "var(--text)", backgroundColor: "var(--code-background)" },
  ".cm-properties-first": { borderTopLeftRadius: "6px", borderTopRightRadius: "6px" },
  ".cm-properties-last": { borderBottomLeftRadius: "6px", borderBottomRightRadius: "6px" },
  ".cm-properties-label": { color: "var(--syntax)", fontSize: "0.85em", fontWeight: "600" },
  // Cells: a value too long for the line is cut, as it cannot wrap.
  ".cm-property-key, .cm-property-value, .cm-property-item": {
    display: "inline-block",
    overflow: "hidden",
    textOverflow: "ellipsis",
    verticalAlign: "top",
  },
  ".cm-property-key": { width: "var(--key-width)", "--frontmatter-text": "var(--quote)" },
  ".cm-property-value": { maxWidth: "calc(100% - var(--key-width) - 1em)" },
  ".cm-property-item": {
    maxWidth: "calc(100% - var(--key-width) - 1em)",
    backgroundColor: "var(--code-background)",
    borderRadius: "10px",
    padding: "0 0.5em",
    marginRight: "0.3em",
  },
  // CodeMirror's 6px before every line, and the keys' column.
  ".cm-line.cm-property-items": { paddingLeft: "calc(var(--key-width) + 6px)" },
  ".cm-frontmatter-unreadable": { textDecoration: "underline wavy var(--error)", textUnderlineOffset: "3px" },
});
