import type { SyntaxNode } from "@lezer/common";
import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import { type EditorState, RangeSetBuilder, StateEffect, StateField, type Text } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { styleTags, tags } from "@lezer/highlight";
import type { InlineContext, MarkdownConfig } from "@lezer/markdown";

// `[[name]]`, `[[name|alias]]` and the Embed `![[file]]`, on one line, as Obsidian writes them.
// The app resolves what is between the brackets; the editor only finds them and reports clicks.
const bracket = 91; // [
const closing = 93; // ]
const bang = 33; // !
const backtick = 96; // `
const newline = 10;

/** Where the `]]` closing a link opened at `start` ends, or -1: not on this line, not past code. */
function closingEnd(cx: InlineContext, start: number): number {
  for (let pos = start; pos < cx.end - 1; pos++) {
    const char = cx.char(pos);
    if (char === newline || char === backtick) return -1;
    if (char === closing && cx.char(pos + 1) === closing) return pos === start ? -1 : pos + 2;
  }
  return -1;
}

function link(name: "Wikilink" | "Embed", cx: InlineContext, pos: number, inner: number): number {
  // `[[[x]]`: not a link at the first bracket; the parser tries again at the next.
  if (cx.char(inner) === bracket) return -1;
  const end = closingEnd(cx, inner);
  if (end === -1) return -1;
  return cx.addElement(
    cx.elt(name, pos, end, [cx.elt("WikilinkMark", pos, inner), cx.elt("WikilinkMark", end - 2, end)]),
  );
}

export const wikilinks: MarkdownConfig = {
  defineNodes: ["Wikilink", "Embed", "WikilinkMark"],
  props: [styleTags({ "Wikilink Embed": tags.link, WikilinkMark: tags.processingInstruction })],
  parseInline: [
    {
      name: "Embed",
      before: "Image",
      parse: (cx, next, pos) =>
        next === bang && cx.char(pos + 1) === bracket && cx.char(pos + 2) === bracket ? link("Embed", cx, pos, pos + 3) : -1,
    },
    {
      name: "Wikilink",
      before: "Link",
      parse: (cx, next, pos) => (next === bracket && cx.char(pos + 1) === bracket ? link("Wikilink", cx, pos, pos + 2) : -1),
    },
  ],
};

/** What is between a Wikilink's or an Embed's brackets; null for any other node. */
function innerOf(node: { name: string; from: number; to: number }, doc: Text): string | null {
  if (node.name !== "Wikilink" && node.name !== "Embed") return null;
  return doc.sliceString(node.from + (node.name === "Embed" ? 3 : 2), node.to - 2);
}

/** The Wikilink or Embed at a document position, with what is between its brackets. */
function linkAt(view: EditorView, pos: number): { from: number; to: number; inner: string } | null {
  for (let node: SyntaxNode | null = syntaxTree(view.state).resolveInner(pos, 1); node; node = node.parent) {
    const inner = innerOf(node, view.state.doc);
    if (inner !== null) return { from: node.from, to: node.to, inner };
  }
  return null;
}

/**
 * A click on a Wikilink or Embed follows it, unless the cursor was already in that link: then
 * the click places the cursor, to edit it, as in Obsidian's live preview. Followed on release
 * and only without a drag, so a selection can start on a link; not with Control (the context
 * menu) nor while an input method is composing.
 */
export function followWikilinks(follow: (inner: string) => void) {
  let pressed: { inner: string; x: number; y: number } | null = null;
  return EditorView.domEventHandlers({
    mousedown(event, view) {
      pressed = null;
      if (event.button !== 0 || event.shiftKey || event.altKey || event.ctrlKey || view.composing) return false;
      // Only over the text itself: not beside it, where the nearest position would be a link.
      const pos = view.posAtCoords({ x: event.clientX, y: event.clientY }, false);
      if (pos === null) return false;
      const found = linkAt(view, pos);
      if (!found) return false;
      const head = view.state.selection.main.head;
      if (head >= found.from && head <= found.to && view.hasFocus) return false;
      pressed = { inner: found.inner, x: event.clientX, y: event.clientY };
      return false;
    },
    click(event, view) {
      const target = pressed;
      pressed = null;
      if (!target || event.detail > 1 || view.composing) return false;
      if (Math.abs(event.clientX - target.x) > 4 || Math.abs(event.clientY - target.y) > 4) return false;
      if (!view.state.selection.main.empty) return false;
      follow(target.inner);
      return true;
    },
  });
}

/**
 * Every Wikilink and Embed in the document, what is between their brackets, once each; null
 * while the document is not parsed to the end (a very long Note): ask again later.
 */
export function linkInners(state: EditorState): string[] | null {
  // At most a few milliseconds of parsing here: the rest goes on in the background.
  const tree = ensureSyntaxTree(state, state.doc.length, 20);
  if (!tree) return null;
  const inners = new Set<string>();
  tree.iterate({
    enter(node) {
      const inner = innerOf(node, state.doc);
      if (inner === null) return;
      inners.add(inner);
      return false;
    },
  });
  return [...inners].sort();
}

const setUnresolved = StateEffect.define<Set<string>>();

/** The links the app said lead nowhere, by what is between their brackets. */
const unresolved = StateField.define<Set<string>>({
  create: () => new Set(),
  update(value, transaction) {
    for (const effect of transaction.effects) if (effect.is(setUnresolved)) value = effect.value;
    return value;
  },
});

export function markUnresolved(view: EditorView, inners: string[]) {
  view.dispatch({ effects: setUnresolved.of(new Set(inners)) });
}

const unresolvedMark = Decoration.mark({ class: "cm-unresolved" });

function unresolvedDecorations(view: EditorView): DecorationSet {
  const set = view.state.field(unresolved);
  if (set.size === 0) return Decoration.none;
  const builder = new RangeSetBuilder<Decoration>();
  for (const { from, to } of view.visibleRanges) {
    syntaxTree(view.state).iterate({
      from,
      to,
      enter(node) {
        const inner = innerOf(node, view.state.doc);
        if (inner === null) return;
        if (set.has(inner)) builder.add(node.from, node.to, unresolvedMark);
        return false;
      },
    });
  }
  return builder.finish();
}

/** Unresolved links drawn faded, as in Obsidian; the set comes from the app. */
export const unresolvedLinks = [
  unresolved,
  ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) {
        this.decorations = unresolvedDecorations(view);
      }
      update(update: ViewUpdate) {
        const changed = update.startState.field(unresolved) !== update.state.field(unresolved);
        // Also as the background parse reaches further: links it finds get their mark.
        const parsed = syntaxTree(update.startState) !== syntaxTree(update.state);
        if (changed || parsed || update.docChanged || update.viewportChanged) this.decorations = unresolvedDecorations(update.view);
      }
    },
    { decorations: (plugin) => plugin.decorations },
  ),
  EditorView.baseTheme({ ".cm-unresolved": { opacity: "0.55" } }),
];
