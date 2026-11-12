import { Prec, type Range } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";

// CodeMirror draws the selection in a layer behind the text: whatever has a background of its own
// (inline code, a code block, a table's header, a highlight) hides it, and selected code looked
// unselected. The selected text is also marked with the selection's colour, innermost: at the
// highest precedence CodeMirror draws the mark inside the others. Outside, it would cut a table's
// cell, an inline box, into pieces around the selection.

const selected = Decoration.mark({ class: "cm-selected-text" });

function decorations(view: EditorView): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  for (const range of view.state.selection.ranges) {
    if (range.empty) continue;
    // Only what is in view: ⌘A on a long Note marks no more than a screen.
    for (const visible of view.visibleRanges) {
      const from = Math.max(range.from, visible.from);
      const to = Math.min(range.to, visible.to);
      if (from < to) ranges.push(selected.range(from, to));
    }
  }
  return Decoration.set(ranges, true);
}

export const selectionOverBackgrounds = [
  Prec.highest(
    ViewPlugin.fromClass(
      class {
        decorations: DecorationSet;
        constructor(view: EditorView) {
          this.decorations = decorations(view);
        }
        update(update: ViewUpdate) {
          if (update.selectionSet || update.docChanged || update.viewportChanged) this.decorations = decorations(update.view);
        }
      },
      { decorations: (plugin) => plugin.decorations },
    ),
  ),
  EditorView.baseTheme({
    // Over the page's background the layer behind already shows the selection: nothing to add.
    // Over a background of its own, the selection a little stronger than plain, so it stands out
    // from code's background in every theme.
    ".cm-inline-code .cm-selected-text, .cm-code-block .cm-selected-text, .cm-table-head .cm-selected-text, .cm-selected-text .cm-inline-code": {
      backgroundColor: "color-mix(in srgb, var(--selection), var(--text) 18%)",
    },
  }),
];
