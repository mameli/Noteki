import { strict as assert } from "node:assert";
import { test } from "node:test";
import { type EditorSettings, settingsVariables } from "../src/settings";

const standard: EditorSettings = { theme: "github", size: 15, lineLength: 100, limitsLineLength: true };

test("the font, its size and the line length become the text's CSS variables, the length in half ems", () => {
  assert.deepEqual(settingsVariables({ ...standard, font: "Iowan Old Style", monoFont: "JetBrains Mono", size: 17, lineLength: 80 }), {
    "--text-font": '"Iowan Old Style", var(--system-font)',
    "--mono": '"JetBrains Mono", var(--system-mono)',
    "--text-size": "17px",
    "--line-width": "calc(80 * 0.5 * var(--text-size))",
  });
});

test("without a font, the system's, for text and code", () => {
  assert.equal(settingsVariables(standard)["--text-font"], "var(--system-font)");
  assert.equal(settingsVariables(standard)["--mono"], "var(--system-mono)");
});

test("without a limit, lines are as wide as the window", () => {
  assert.equal(settingsVariables({ ...standard, limitsLineLength: false })["--line-width"], "none");
});

test("a font's name cannot break out of its quotes", () => {
  assert.equal(settingsVariables({ ...standard, font: 'A"; color: red' })["--text-font"], '"A\\"; color: red", var(--system-font)');
});
