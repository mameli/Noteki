import { strict as assert } from "node:assert";
import { test } from "node:test";
import { syntaxTree } from "@codemirror/language";
import { renderedFootnotes } from "../src/livepreview";
import { shown, stateOf } from "./helpers";

const note = "Text[^b] and more[^a], again[^b].\n\n[^a]: First.\n[^b]: Second,\n  continued.\n\nAfter.";

test("references and definitions are parsed; a definition continues on its indented lines", () => {
  const state = stateOf(`${note}‸`);
  const names: string[] = [];
  syntaxTree(state).iterate({ enter: (node) => void (node.name.startsWith("Footnote") && names.push(`${node.name}:${state.sliceDoc(node.from, node.to)}`)) });
  assert.equal(names.includes("FootnoteReference:[^b]"), true);
  assert.equal(names.includes("FootnoteDefinition:[^a]: First."), true);
  assert.equal(names.includes("FootnoteDefinition:[^b]: Second,\n  continued."), true);
});

test("references are numbered in the order they first appear, definitions take their number", () => {
  const drawn = renderedFootnotes(stateOf(`${note}‸`)).map(({ label, number, definition }) => `${definition ? "def" : "ref"} ${label} ${number}`);
  assert.deepEqual(drawn, ["ref b 1", "ref a 2", "ref b 1", "def a 2", "def b 1"]);
});

test("a footnote the cursor is on is written; its text stays Markdown", () => {
  const drawn = renderedFootnotes(stateOf("Text[^‸a].\n\n[^a]: **bold**"));
  assert.deepEqual(drawn.map((note) => note.definition), [true]);
  assert.equal(shown("Text[^a].\n\n[^a]: **bold**\n‸"), "Text[^a].\n\n[^a]: bold\n");
});

test("brackets that are no footnote stay as they are", () => {
  assert.equal(renderedFootnotes(stateOf("[^ a] and [^] and [x]\n‸")).length, 0);
});
