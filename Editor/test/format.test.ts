import { strict as assert } from "node:assert";
import { test } from "node:test";
import type { EditorState } from "@codemirror/state";
import { type Formatting, formatting } from "../src/format";
import { stateOf } from "./helpers";

/** The text after `action`, `‸` where the cursor is, `‸…‸` around the selection. */
function formatted(marked: string, action: Formatting, url?: string): string {
  const state = stateOf(marked);
  return written(state.update(formatting(state, action, url)).state);
}

function written(state: EditorState): string {
  const { anchor, head } = state.selection.main;
  const [from, to] = [Math.min(anchor, head), Math.max(anchor, head)];
  const text = state.doc.toString();
  return from === to ? text.slice(0, from) + "‸" + text.slice(from) : text.slice(0, from) + "‸" + text.slice(from, to) + "‸" + text.slice(to);
}

test("bold goes around the selection, which stays on the text", () => {
  assert.equal(formatted("a ‸word‸ b", "bold"), "a **‸word‸** b");
});

test("with nothing selected, around the word at the cursor", () => {
  assert.equal(formatted("a wo‸rd b", "italic"), "a *wo‸rd* b");
});

test("with no word, two marks and the cursor between them", () => {
  assert.equal(formatted("a ‸", "strikethrough"), "a ~~‸~~");
});

test("bold again takes the marks away, selected inside or with them", () => {
  assert.equal(formatted("a **‸word‸** b", "bold"), "a ‸word‸ b");
  assert.equal(formatted("a ‸**word**‸ b", "bold"), "a ‸word‸ b");
  assert.equal(formatted("a **wo‸rd** b", "bold"), "a wo‸rd b");
});

test("italic inside bold is added, not mistaken for bold's marks", () => {
  assert.equal(formatted("**‸word‸**", "italic"), "***‸word‸***");
  assert.equal(formatted("***‸word‸***", "italic"), "**‸word‸**");
});

test("code and a Comment of the whole line", () => {
  assert.equal(formatted("‸x‸", "code"), "`‸x‸`");
  assert.equal(formatted("a no‸te", "comment"), "%%a no‸te%%");
});

test("a link of the word: with the pasteboard's address the cursor goes after it", () => {
  assert.equal(formatted("see Git‸Hub", "link", "https://github.com"), "see [GitHub](https://github.com)‸");
  assert.equal(formatted("see ‸GitHub‸", "link"), "see [GitHub](‸)");
  assert.equal(formatted("see ‸", "link"), "see [‸]()");
});

test("a heading of the line, of another level, or taken away", () => {
  assert.equal(formatted("Ti‸tle", "heading2"), "## Ti‸tle");
  assert.equal(formatted("# Ti‸tle", "heading3"), "### Ti‸tle");
  assert.equal(formatted("## Ti‸tle", "heading2"), "Ti‸tle");
});

test("a task: from text, from a list item, then ticked and cleared", () => {
  assert.equal(formatted("  bu‸y milk", "task"), "  - [ ] bu‸y milk");
  assert.equal(formatted("- bu‸y milk", "task"), "- [ ] bu‸y milk");
  assert.equal(formatted("- [ ] bu‸y milk", "task"), "- [x] bu‸y milk");
  assert.equal(formatted("1. [x] bu‸y milk", "task"), "1. [ ] bu‸y milk");
});

test("a quote of the lines, taken away again", () => {
  assert.equal(formatted("‸one\ntwo‸", "quote"), "> ‸one\n> two‸");
  assert.equal(formatted("> o‸ne", "quote"), "o‸ne");
});

test("a list of the lines, numbered from 1; another kind changes kind, the same goes", () => {
  assert.equal(formatted("‸one\ntwo‸", "numberedList"), "1. ‸one\n2. two‸");
  assert.equal(formatted("- o‸ne", "numberedList"), "1. o‸ne");
  assert.equal(formatted("  - [ ] o‸ne", "bulletList"), "  - o‸ne");
  assert.equal(formatted("- o‸ne", "bulletList"), "o‸ne");
});

test("every line of the selection", () => {
  assert.equal(formatted("‸one\ntwo‸", "heading1"), "# ‸one\n# two‸");
});
