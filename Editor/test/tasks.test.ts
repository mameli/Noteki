import { strict as assert } from "node:assert";
import { test } from "node:test";
import { renderedTasks } from "../src/livepreview";
import { toggleTask } from "../src/tasks";
import { shown, stateOf as state } from "./helpers";

test("a task shows a checkbox in place of its bullet and brackets, ticked as the file says", () => {
  const text = "- [ ] open\n* [x] done\n  + [X] nested\n‸";
  assert.equal(shown(text), " open\n done\n   nested\n");
  assert.deepEqual(
    renderedTasks(state(text)).map((task) => task.checked),
    [false, true, true],
  );
});

test("a numbered task keeps its number; a quote's task is a task", () => {
  assert.equal(shown("1. [ ] first\n> - [ ] quoted\n‸"), "1.  first\n>  quoted\n");
});

test("the cursor on the brackets or the bullet shows them, the cursor in the text does not", () => {
  assert.equal(shown("- [‸ ] open"), "- [ ] open");
  assert.equal(shown("‸- [ ] open"), "- [ ] open");
  assert.equal(shown("- [ ] op‸en"), " open");
  assert.equal(renderedTasks(state("- [‸ ] open")).length, 0);
});

test("brackets that are no task stay as written", () => {
  assert.equal(shown("[ ] not in a list\n- [y] other\n- [ ]\n‸"), "[ ] not in a list\n- [y] other\n- [ ]\n");
});

test("a click on a checkbox writes [x] or [ ] in the file, and nothing else", () => {
  const before = state("- [ ] open\n- [X] done\n‸");
  const [open, done] = renderedTasks(before);

  const ticked = before.update(toggleTask(before, open.at)!).state;
  assert.equal(ticked.doc.toString(), "- [x] open\n- [X] done\n");
  const unticked = ticked.update(toggleTask(ticked, done.at)!).state;
  assert.equal(unticked.doc.toString(), "- [x] open\n- [ ] done\n");
  assert.equal(toggleTask(before, before.doc.length), null);
});
