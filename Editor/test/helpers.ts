import { ensureSyntaxTree } from "@codemirror/language";
import { EditorSelection, EditorState } from "@codemirror/state";
import { noteLanguage } from "../src/language";
import { hiddenRanges } from "../src/livepreview";

/** A Note's state for `marked`: `‸` is the cursor, `‸…‸` a selection; at the end without one. */
export function stateOf(marked: string): EditorState {
  const first = marked.indexOf("‸");
  const second = marked.indexOf("‸", first + 1);
  const text = marked.replaceAll("‸", "");
  const anchor = first === -1 ? text.length : first;
  const head = second === -1 ? anchor : second - 1;
  const state = EditorState.create({
    doc: text,
    selection: EditorSelection.single(anchor, head),
    extensions: [noteLanguage],
  });
  ensureSyntaxTree(state, state.doc.length, 5000);
  return state;
}

/** The text as Live preview shows it: what is hidden is left out, line breaks kept. */
export function shown(marked: string): string {
  const state = stateOf(marked);
  const text = state.doc.toString();
  let result = "";
  let at = 0;
  for (const { from, to } of hiddenRanges(state)) {
    result += text.slice(at, from);
    at = to;
  }
  return result + text.slice(at);
}
