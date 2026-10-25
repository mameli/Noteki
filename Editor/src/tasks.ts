import { syntaxTree } from "@codemirror/language";
import type { EditorState, TransactionSpec } from "@codemirror/state";
import { EditorView, WidgetType } from "@codemirror/view";
import type { Span } from "./text";

// Task lists: `- [ ] task` shows a checkbox in place of its bullet and brackets (livepreview.ts);
// a click on it writes `[x]` or `[ ]` in the file, an edit like any other: undone with ⌘Z, saved
// by the app.

/** Whether a task's brackets, `[ ]` or `[x]`, say it is done. */
export function isTicked(state: EditorState, marker: Span): boolean {
  return state.sliceDoc(marker.from + 1, marker.from + 2) !== " ";
}

/** The click on the checkbox drawn at `at`: its task's brackets ticked or cleared; null if none. */
export function toggleTask(state: EditorState, at: number): TransactionSpec | null {
  const line = state.doc.lineAt(at);
  let marker: Span | null = null;
  syntaxTree(state).iterate({
    from: line.from,
    to: line.to,
    enter(node) {
      if (marker) return false;
      if (node.name === "TaskMarker" && node.from >= line.from) marker = { from: node.from, to: node.to };
      return undefined;
    },
  });
  if (!marker) return null;
  const inside = (marker as Span).from + 1;
  return { changes: { from: inside, to: inside + 1, insert: isTicked(state, marker) ? " " : "x" }, userEvent: "input" };
}

export class CheckboxWidget extends WidgetType {
  constructor(readonly checked: boolean) {
    super();
  }

  eq(other: CheckboxWidget) {
    return other.checked === this.checked;
  }

  toDOM(view: EditorView) {
    const box = document.createElement("input");
    box.type = "checkbox";
    box.className = "cm-task-checkbox";
    box.checked = this.checked;
    box.tabIndex = -1;
    // The cursor stays where it was: the checkbox does not take it, nor show the brackets.
    box.addEventListener("mousedown", (event) => event.preventDefault());
    // On click, which VoiceOver sends too. Drawn again from the file once it changed.
    box.addEventListener("click", (event) => {
      event.preventDefault();
      if (event.button !== 0 || !view.state.facet(EditorView.editable)) return;
      const spec = toggleTask(view.state, view.posAtDOM(box));
      if (spec) view.dispatch(spec);
    });
    return box;
  }

  ignoreEvent() {
    return true;
  }
}

export const taskTheme = EditorView.baseTheme({
  ".cm-task-checkbox": { margin: "0 0.15em 0 0", verticalAlign: "-0.1em", width: "0.95em", height: "0.95em", cursor: "default" },
  // A done task is crossed out and faded, as in Obsidian.
  ".cm-task-done": { textDecoration: "line-through", color: "var(--quote)" },
});
