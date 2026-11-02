import { strict as assert } from "node:assert";
import { test } from "node:test";
import { formulas } from "../src/math";
import { stateOf } from "./helpers";

const found = (text: string) => formulas(stateOf(text)).map(({ tex, display }) => [tex, display]);

test("$…$ in the text and $$…$$ on its own, across lines too, are formulas", () => {
  assert.deepEqual(found("Area $\\pi r^2$ here.\n\n$$\n\\int_0^1 x\\,dx\n$$\n"), [
    ["\\pi r^2", false],
    ["\\int_0^1 x\\,dx", true],
  ]);
});

test("prices, spaces inside the dollars, code and escaped dollars are not", () => {
  assert.deepEqual(found("It costs $5 and $10.\n\nA $ b $ c.\n\n`$x$` and \\$y\\$.\n"), []);
});

test("a block of LaTeX is not Markdown: a = on its own line is no Heading's underline", () => {
  const text = "Matrix:\n\n$$\nA\n=\nB\n$$\n\nAfter";
  assert.deepEqual(found(text), [["A\n=\nB", true]]);
  assert.equal(stateOf(text).doc.toString().includes("$$"), true);
});

test("a block of LaTeX ends a paragraph, as a fence does; one never closed is left as written", () => {
  assert.deepEqual(found("Text\n$$x^2$$\nmore"), [["x^2", true]]);
  assert.deepEqual(found("$$\nx^2\n\nText"), []);
});

test("a dollar's formula does not reach into code", () => {
  assert.deepEqual(found("costa $5 e $10, un dollaro \\$ e il codice `$x$`."), []);
});
