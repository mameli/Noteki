import { strict as assert } from "node:assert";
import { test } from "node:test";
import { history, undo } from "@codemirror/commands";
import { EditorState, type Transaction } from "@codemirror/state";
import { insertion } from "../src/insert";
import { stateOf } from "./helpers";

function inserted(marked: string, text: string, at?: number): EditorState {
  return stateOf(marked).update(insertion(stateOf(marked), text, at)).state;
}

function marked(state: EditorState): string {
  const { from, to } = state.selection.main;
  const text = state.doc.toString();
  return from === to ? text.slice(0, from) + "‸" + text.slice(from) : text.slice(0, from) + "‸" + text.slice(from, to) + "‸" + text.slice(to);
}

test("a Pasted image's Embed goes at the cursor, the cursor after it", () => {
  assert.equal(marked(inserted("Before ‸after", "![[Pasted image 1.png]]")), "Before ![[Pasted image 1.png]]‸after");
});

test("it takes the place of the selection", () => {
  assert.equal(marked(inserted("Keep ‸this‸ out", "![[a.png]]")), "Keep ![[a.png]]‸ out");
});

test("dropped files go where they are dropped, whatever is selected", () => {
  assert.equal(marked(inserted("‸one‸ two", "![[a.png]]\n![[b.png]]", 4)), "one ![[a.png]]\n![[b.png]]‸two");
});

test("one undo takes the Embed out again", () => {
  const state = EditorState.create({ doc: "text", extensions: [history()], selection: { anchor: 4 } });
  const after = state.update(insertion(state, " ![[a.png]]")).state;
  let undone: EditorState = after;
  undo({ state: after, dispatch: (transaction: Transaction) => (undone = transaction.state) });
  assert.equal(undone.doc.toString(), "text");
});
