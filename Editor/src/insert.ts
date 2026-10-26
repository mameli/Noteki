import type { EditorState, TransactionSpec } from "@codemirror/state";

/**
 * Text the app writes for the user, the Embeds of a Pasted image or of dropped files: at `at`
 * where files were dropped, otherwise in place of the selection, with the cursor after it. One
 * step of undo, as typing it would be.
 */
export function insertion(state: EditorState, text: string, at?: number): TransactionSpec {
  const { from, to } = at === undefined ? state.selection.main : { from: at, to: at };
  return {
    changes: { from, to, insert: text },
    selection: { anchor: from + text.length },
    scrollIntoView: true,
    userEvent: at === undefined ? "input.paste" : "input.drop",
  };
}
