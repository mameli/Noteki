import { strict as assert } from "node:assert";
import { test } from "node:test";
import type { EditorState } from "@codemirror/state";
import { bodyStart, type FrontmatterReading, frontmatterText, setFrontmatter } from "../src/frontmatter";
import { renderedFrontmatter } from "../src/livepreview";
import { shown, stateOf } from "./helpers";

const note = '---\ntitle: "Retro"\ntags: [a, b]\nlist:\n  - x\nempty:\n---\nBody';

/** What the app answers for `note` (NotekiCore's Frontmatter, ticket 23). */
const reading: FrontmatterReading = {
  properties: [
    { key: "title", keySpan: { line: 2, from: 0, to: 5 }, value: { kind: "text", span: { line: 2, from: 8, to: 13 } } },
    { key: "tags", keySpan: { line: 3, from: 0, to: 4 }, value: { kind: "list", items: [{ line: 3, from: 7, to: 8 }, { line: 3, from: 10, to: 11 }] } },
    { key: "list", keySpan: { line: 4, from: 0, to: 4 }, value: { kind: "list", items: [{ line: 5, from: 4, to: 5 }] } },
    { key: "empty", keySpan: { line: 6, from: 0, to: 5 }, value: { kind: "none" } },
  ],
};

/** The Note's state once the app answered about `text`. */
function answered(state: EditorState, answer: FrontmatterReading | null, text = frontmatterText(state)!): EditorState {
  return state.update({ effects: setFrontmatter.of({ text, reading: answer }) }).state;
}

function shownWith(marked: string, answer: FrontmatterReading | null, text?: string): string {
  return shown(marked, (state) => answered(state, answer, text));
}

test("the editor asks about its Frontmatter from the first --- line to the last, and only once it closes", () => {
  assert.equal(frontmatterText(stateOf(note)), '---\ntitle: "Retro"\ntags: [a, b]\nlist:\n  - x\nempty:\n---');
  assert.equal(frontmatterText(stateOf("---\na: b\n")), null);
  assert.equal(frontmatterText(stateOf("# Title\n---\na: b\n---")), null);
});

test("away from the cursor, the Properties are a table: keys, then values as written, a list's items one per line or side by side", () => {
  assert.equal(shownWith(note + "‸", reading), "\ntitleRetro\ntagsab\nlist\nx\nempty\n\nBody");
  const table = renderedFrontmatter(answered(stateOf(note + "‸"), reading))!;
  assert.equal(table.keyWidth, 5);
  assert.deepEqual(table.items.map(({ from, to }) => note.slice(from, to)), ["a", "b", "x"]);
  assert.deepEqual(table.values.map(({ from, to }) => note.slice(from, to)), ["Retro"]);
});

test("a comment on a line of its own is an empty row of the table", () => {
  const commented = "---\ntags:\n  - a\n  # old\n  - b\n---\nBody‸";
  const answer: FrontmatterReading = {
    properties: [{ key: "tags", keySpan: { line: 2, from: 0, to: 4 }, value: { kind: "list", items: [{ line: 3, from: 4, to: 5 }, { line: 5, from: 4, to: 5 }] } }],
  };
  assert.equal(shownWith(commented, answer), "\ntags\na\n\nb\n\nBody");
});

test("the cursor in the Frontmatter shows its YAML; leaving draws the table again", () => {
  assert.equal(shownWith('---\ntitle: "Re‸tro"\ntags: [a, b]\nlist:\n  - x\nempty:\n---\nBody', reading), note);
  assert.equal(shownWith("‸" + note, reading), note);
  assert.equal(renderedFrontmatter(answered(stateOf("‸" + note), reading)), null);
});

test("until the app answers about the Frontmatter as it is now, it is shown as written", () => {
  assert.equal(shownWith(note + "‸", reading, "---\nold: x\n---"), note);
  assert.equal(shown(note + "‸"), note);
});

test("YAML the app cannot read is shown as written, with a mark on the line where reading stopped", () => {
  const broken = "---\na: b\nc:\n  d: e\n---\nBody‸";
  assert.equal(shownWith(broken, { unreadable: 4 }), broken.replace("‸", ""));
  const state = answered(stateOf(broken), { unreadable: 4 });
  assert.equal(renderedFrontmatter(state)?.unreadableLine, 4);
});

test("a Note opens with the cursor where its text starts, after the Frontmatter", () => {
  assert.equal(bodyStart(stateOf(note)), note.indexOf("Body"));
  assert.equal(bodyStart(stateOf("---\na: b\n---")), 12);
  assert.equal(bodyStart(stateOf("# Title")), 0);
});
