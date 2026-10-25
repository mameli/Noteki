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
  assert.deepEqual(table.items.map(({ from, to }) => note.slice(from, to)), ["x"]);
  assert.deepEqual(table.tags.chips.map((chip) => chip.name), ["a", "b"]);
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

test("the `tags` Property's values are chips, a field to add one after the last; without it, the field is on the table's top", () => {
  const tagged = "---\ntags:\n  - meeting\n  - '#client/axo'\nx: [a, b]\n---\nBody‸";
  const answer: FrontmatterReading = {
    properties: [
      { key: "tags", keySpan: { line: 2, from: 0, to: 4 }, value: { kind: "list", items: [{ line: 3, from: 4, to: 11 }, { line: 4, from: 5, to: 16 }] } },
      { key: "x", keySpan: { line: 5, from: 0, to: 1 }, value: { kind: "list", items: [{ line: 5, from: 4, to: 5 }, { line: 5, from: 7, to: 8 }] } },
    ],
  };
  const table = renderedFrontmatter(answered(stateOf(tagged), answer))!;
  assert.deepEqual(table.tags.chips.map((chip) => chip.name), ["meeting", "client/axo"]);
  assert.equal(table.tags.addAt, tagged.indexOf("'\nx:") + 1);
  assert.deepEqual(table.items.map(({ from, to }) => tagged.slice(from, to)), ["a", "b"]);

  const untagged = "---\ntitle: x\n---\nBody‸";
  const titled: FrontmatterReading = { properties: [{ key: "title", keySpan: { line: 2, from: 0, to: 5 }, value: { kind: "text", span: { line: 2, from: 7, to: 8 } } }] };
  assert.equal(renderedFrontmatter(answered(stateOf(untagged), titled))!.tags.addAt, 3);
});

test("Tags written in one text are a chip each", () => {
  const text = "---\ntags: a b, c\n---\nBody‸";
  const answer: FrontmatterReading = { properties: [{ key: "tags", keySpan: { line: 2, from: 0, to: 4 }, value: { kind: "text", span: { line: 2, from: 6, to: 12 } } }] };
  const table = renderedFrontmatter(answered(stateOf(text), answer))!;
  assert.deepEqual(table.tags.chips.map((chip) => [chip.name, text.slice(chip.from, chip.to)]), [["a", "a"], ["b", "b"], ["c", "c"]]);
  assert.equal(shown(text, (state) => answered(state, answer)), "\ntagsabc\n\nBody");
});

test("an item of the list is one chip, whatever it holds", () => {
  const text = "---\ntags:\n  - client a\n---\nBody‸";
  const answer: FrontmatterReading = { properties: [{ key: "tags", keySpan: { line: 2, from: 0, to: 4 }, value: { kind: "list", items: [{ line: 3, from: 4, to: 12 }] } }] };
  assert.deepEqual(renderedFrontmatter(answered(stateOf(text), answer))!.tags.chips.map((chip) => chip.name), ["client a"]);
});
