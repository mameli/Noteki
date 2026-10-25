import { strict as assert } from "node:assert";
import { test } from "node:test";
import { renderedCallouts } from "../src/livepreview";
import { shown, stateOf as state } from "./helpers";

test("a callout shows its title and text without its > marks and its [!type]", () => {
  const text = "> [!note] Read this\n> first line\n>\n> **second**\n\nAfter‸";
  assert.equal(shown(text), "Read this\nfirst line\n\nsecond\n\nAfter");
  const [callout] = renderedCallouts(state(text));
  assert.equal(callout.kind, "note");
  assert.equal(callout.typeTitle, null);
  assert.equal(state(text).sliceDoc(callout.from, callout.to), "> [!note] Read this\n> first line\n>\n> **second**");
});

test("without a title, the type is its title; the colour follows the type, as in Obsidian", () => {
  const callouts = renderedCallouts(state("> [!warning]\n> text\n\n> [!faq] Q\n\n> [!Caution]\n\n> [!custom-thing]\n\n‸"));
  assert.deepEqual(
    callouts.map((callout) => [callout.kind, callout.typeTitle]),
    [["warning", "Warning"], ["question", null], ["warning", "Caution"], ["note", "Custom-thing"]],
  );
  assert.equal(shown("> [!warning]\n> text\n‸"), "\ntext\n");
});

test("a foldable callout is shown open", () => {
  assert.equal(shown("> [!tip]- Folded\n> inside\n‸"), "Folded\ninside\n");
  assert.equal(shown("> [!tip]+\n> inside\n‸"), "\ninside\n");
});

test("the cursor shows the marks of its line only, and the callout stays drawn", () => {
  assert.equal(shown("> [!note] Title\n> bo‸dy"), "Title\n> body");
  assert.equal(shown("> [!note] Ti‸tle\n> body"), "> [!note] Title\nbody");
  const [written] = renderedCallouts(state("> [!warning]‸\n> body"));
  assert.equal(written.typeTitle, null);
});

test("a line continuing the callout without > is its own", () => {
  const [callout] = renderedCallouts(state("> [!note] T\ncontinued\n\n‸"));
  assert.equal(state("> [!note] T\ncontinued\n\n‸").sliceDoc(callout.from, callout.to), "> [!note] T\ncontinued");
});

test("a callout in a list item is drawn as one", () => {
  assert.equal(shown("- > [!note] T\n  > body\n‸"), "- T\n  body\n");
});

test("a quote is no callout, nor a callout inside a quote", () => {
  assert.equal(shown("> quote\n\n> > [!note] nested\n‸"), "> quote\n\n> > [!note] nested\n");
  assert.equal(renderedCallouts(state("> quote\n\n> > [!note] nested\n‸")).length, 0);
});
