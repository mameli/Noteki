import { EditorState, type Range } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, highlightActiveLine, ViewPlugin, type ViewUpdate } from "@codemirror/view";

// Focus mode and Typewriter mode (View menu), as in MarkText: one fades every line but the
// paragraph being written, the other keeps the line being written in the middle of the window.

/** The lines of the paragraph holding `pos`: up and down to the nearest blank line. */
export function paragraphAround(state: EditorState, pos: number): { from: number; to: number } {
  const doc = state.doc;
  let first = doc.lineAt(pos);
  let last = first;
  if (first.text.trim() === "") return { from: first.from, to: first.to };
  while (first.number > 1 && doc.line(first.number - 1).text.trim() !== "") first = doc.line(first.number - 1);
  while (last.number < doc.lines && doc.line(last.number + 1).text.trim() !== "") last = doc.line(last.number + 1);
  return { from: first.from, to: last.to };
}

const faded = Decoration.line({ class: "cm-faded" });

function fadedLines(view: EditorView): DecorationSet {
  const kept = paragraphAround(view.state, view.state.selection.main.head);
  const ranges: Range<Decoration>[] = [];
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = view.state.doc.lineAt(pos);
      if (line.to < kept.from || line.from > kept.to) ranges.push(faded.range(line.from));
      pos = line.to + 1;
    }
  }
  return Decoration.set(ranges);
}

const focusMode = [
  ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) {
        this.decorations = fadedLines(view);
      }
      update(update: ViewUpdate) {
        if (update.docChanged || update.selectionSet || update.viewportChanged) this.decorations = fadedLines(update.view);
      }
    },
    { decorations: (plugin) => plugin.decorations },
  ),
  EditorView.theme({
    ".cm-line": { transition: "opacity 0.2s" },
    ".cm-faded": { opacity: "0.3" },
  }),
];

/**
 * The line of the cursor scrolled to the middle after typing or moving with the keys; not after a
 * click, which would move the text from under the pointer.
 */
const typewriterScroll = EditorState.transactionExtender.of((transaction) => {
  if (!transaction.selection && !transaction.docChanged) return null;
  if (transaction.isUserEvent("select.pointer") || !transaction.isUserEvent("input") && !transaction.isUserEvent("delete") && !transaction.isUserEvent("select") && !transaction.isUserEvent("move")) {
    return null;
  }
  return { effects: EditorView.scrollIntoView(transaction.newSelection.main.head, { y: "center" }) };
});

/**
 * The text starts and ends half a window away, so every line can stand in the middle, the first
 * too; the line being written has a band of colour: it shows the mode is on.
 */
const typewriterMode = [
  typewriterScroll,
  highlightActiveLine(),
  EditorView.theme({
    "&.cm-editor .cm-content": { paddingTop: "45vh", paddingBottom: "50vh" },
    "&.cm-editor .cm-activeLine": { backgroundColor: "rgba(var(--accent-rgb), 0.08)", borderRadius: "3px" },
  }),
];

/** The modes the settings turn on. */
export function writingModes(settings: { focusMode?: boolean; typewriterMode?: boolean } | undefined) {
  return [settings?.focusMode ? focusMode : [], settings?.typewriterMode ? typewriterMode : []];
}
