import { strict as assert } from "node:assert";
import { test } from "node:test";
import { mermaidBlocks } from "../src/mermaid";
import { stateOf } from "./helpers";

test("a mermaid code block is a diagram, from its opening fence to its closing one", () => {
  const text = "Before\n\n```mermaid\nflowchart LR\n  A --> B\n```\n\nAfter";
  const state = stateOf(text);
  const from = text.indexOf("```mermaid");
  assert.deepEqual(mermaidBlocks(state), [{ from, to: text.indexOf("```\n\nAfter") + 3, source: "flowchart LR\n  A --> B" }]);
});

test("other code blocks, and a mermaid block never closed, are code", () => {
  const state = stateOf("```js\nlet a\n```\n\n```mermaid\ngraph TD\n");
  assert.deepEqual(mermaidBlocks(state), []);
});
