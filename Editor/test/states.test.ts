import { strict as assert } from "node:assert";
import { test } from "node:test";
import { history, undo } from "@codemirror/commands";
import { EditorSelection, EditorState, type Transaction } from "@codemirror/state";
import { broughtBack, changeTo } from "../src/states";

test("only the part that differs changes", () => {
  assert.deepEqual(changeTo("one two three", "one 2 three"), { from: 4, to: 7, insert: "2" });
  assert.equal(changeTo("same", "same"), null);
});

test("a Tab's Note brought back keeps its cursor and undo, the file's changes made since applied outside the history", () => {
  let state = EditorState.create({ doc: "Title\nbody", extensions: [history()], selection: EditorSelection.cursor(10) });
  state = state.update({ changes: { from: 10, insert: " typed" }, selection: EditorSelection.cursor(16) }).state;

  // Saved when the Tab was left; then another app wrote a line at the top.
  const back = broughtBack(state, "New line\nTitle\nbody typed");
  assert.equal(back.doc.toString(), "New line\nTitle\nbody typed");
  assert.equal(back.selection.main.head, 25);

  let undone: EditorState = back;
  undo({ state: back, dispatch: (transaction: Transaction) => (undone = transaction.state) });
  assert.equal(undone.doc.toString(), "New line\nTitle\nbody");
});

test("a Tab's Note brought back unchanged is the same state", () => {
  const state = EditorState.create({ doc: "x" });
  assert.equal(broughtBack(state, "x"), state);
});
