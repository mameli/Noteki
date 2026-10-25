import { strict as assert } from "node:assert";
import { test } from "node:test";
import type { EditorState } from "@codemirror/state";
import { type EmbeddedFile, imageWidgets, setEmbeds } from "../src/embeds";
import { hiddenRanges } from "../src/livepreview";
import { linkInners, setUnresolved, unresolved } from "../src/wikilink";
import { stateOf } from "./helpers";

const files: Record<string, EmbeddedFile> = {
  "a.png": { path: "Media/a.png", kind: "image", width: 640, height: 480 },
  "a.png|300": { path: "Media/a.png", kind: "image", width: 640, height: 480 },
  "Media/b c.png": { path: "Media/b c.png", kind: "image", width: 10, height: 20 },
  "Doc.pdf": { path: "Doc.pdf", kind: "pdf" },
  "Doc.pdf|the contract": { path: "Doc.pdf", kind: "pdf" },
};

/** The state for `marked` with the app's answer about its Embeds. */
function withEmbeds(marked: string): EditorState {
  const state = stateOf(marked);
  return state.update({ effects: setEmbeds.of(files) }).state;
}

/** The text with what is hidden left out. */
function shown(state: EditorState): string {
  const text = state.doc.toString();
  let result = "";
  let at = 0;
  for (const { from, to } of hiddenRanges(state)) {
    result += text.slice(at, from);
    at = to;
  }
  return result + text.slice(at);
}

test("an image Embed is drawn under its line, its source hidden while the cursor is away", () => {
  const state = withEmbeds("Before\n![[a.png]]\nAfter‸");
  assert.equal(shown(state), "Before\n\nAfter");
  assert.deepEqual(
    imageWidgets(state).map((widget) => [widget.at, widget.file.path, widget.width]),
    [[17, "Media/a.png", undefined]],
  );
});

test("with the cursor on it, the source shows and the image stays: nothing moves", () => {
  const state = withEmbeds("Before\n![[a‸.png]]\nAfter");
  assert.equal(shown(state), "Before\n![[a.png]]\nAfter");
  assert.equal(imageWidgets(state).length, 1);
});

test("a Markdown image's URL is decoded; a width after | is the width drawn", () => {
  const state = withEmbeds("![alt](Media/b%20c.png) and ![[a.png|300]]\n‸");
  assert.equal(shown(state), " and \n");
  assert.deepEqual(imageWidgets(state).map((widget) => [widget.file.path, widget.width]), [
    ["Media/b c.png", undefined],
    ["Media/a.png", 300],
  ]);
});

test("a PDF Embed shows its name or its Alias, a link; a Note's Embed is left as written", () => {
  const state = withEmbeds("![[Doc.pdf]] and ![[Doc.pdf|the contract]] and ![[Other note]]\n‸");
  assert.equal(shown(state), "Doc.pdf and the contract and ![[Other note]]\n");
  assert.equal(imageWidgets(state).length, 0);
});

test("the app is asked about Markdown images by their decoded path, not about web images", () => {
  const state = stateOf("![[a.png]] ![x](Media/b%20c.png) ![y](https://example.com/z.png) ![z](<Media/d e.png>)");
  assert.deepEqual(linkInners(state), ["Media/b c.png", "Media/d e.png", "a.png"]);
});

test("a missing image shows its name, as an Unresolved link", () => {
  const state = withEmbeds("![[missing.png]] and ![x](gone.png)\n‸").update({ effects: setUnresolved.of(new Set(["missing.png", "gone.png"])) }).state;
  assert.equal(shown(state), "missing.png and ![x](gone.png)\n");
});

test("an image in a table is not drawn: it would break the table's rows", () => {
  const state = withEmbeds("| a |\n|---|\n| ![[a.png]] |\n\n‸");
  assert.equal(imageWidgets(state).length, 0);
});
