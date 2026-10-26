import { Annotation, type ChangeSpec, type EditorState, Transaction } from "@codemirror/state";

// Each Tab keeps its Note's editor state (ADR 0003): its cursor, its undo history and, through
// main.ts, its scroll. A state brought back takes the file's changes made meanwhile as a reload
// does: only the part that differs, outside the undo history.

/** Marks a change that came from disk: not the user's, so not reported back to the app. */
export const fromDisk = Annotation.define<boolean>();

/** The change that turns `doc` into `text`: only the part that differs, so the cursor maps through it. */
export function changeTo(doc: string, text: string): (ChangeSpec & { from: number; to: number; insert: string }) | null {
  if (doc === text) return null;
  let start = 0;
  while (start < doc.length && start < text.length && doc[start] === text[start]) start++;
  let end = 0;
  while (end < doc.length - start && end < text.length - start && doc[doc.length - 1 - end] === text[text.length - 1 - end]) end++;
  return { from: start, to: doc.length - end, insert: text.slice(start, text.length - end) };
}

/** A kept state with the text the app has now: changed from disk, outside the undo history. */
export function broughtBack(state: EditorState, text: string): EditorState {
  const change = changeTo(state.doc.toString(), text);
  if (!change) return state;
  return state.update({ changes: change, annotations: [fromDisk.of(true), Transaction.addToHistory.of(false)] }).state;
}
