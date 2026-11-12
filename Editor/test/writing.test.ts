import { strict as assert } from "node:assert";
import { test } from "node:test";
import { paragraphAround } from "../src/writing";
import { stateOf } from "./helpers";

test("Focus mode keeps the paragraph of the cursor, from blank line to blank line", () => {
  const state = stateOf("First\n\nOne\nTw‸o\nThree\n\nLast");
  const kept = paragraphAround(state, state.selection.main.head);
  assert.equal(state.sliceDoc(kept.from, kept.to), "One\nTwo\nThree");
  const blank = stateOf("A\n‸\nB");
  assert.equal(blank.sliceDoc(...Object.values(paragraphAround(blank, blank.selection.main.head)) as [number, number]), "");
});
