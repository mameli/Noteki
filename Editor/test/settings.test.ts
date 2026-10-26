import { strict as assert } from "node:assert";
import { test } from "node:test";
import { settingsVariables } from "../src/settings";

test("the font, its size and the line width become the text's CSS variables", () => {
  assert.deepEqual(settingsVariables({ font: "Iowan Old Style", size: 17, lineWidth: 800 }), {
    "--text-font": '"Iowan Old Style", var(--system-font)',
    "--text-size": "17px",
    "--line-width": "800px",
  });
});

test("without a font, the system's", () => {
  assert.equal(settingsVariables({ size: 15, lineWidth: 736 })["--text-font"], "var(--system-font)");
});

test("a font's name cannot break out of its quotes", () => {
  assert.equal(settingsVariables({ font: 'A"; color: red', size: 15, lineWidth: 736 })["--text-font"], '"A\\"; color: red", var(--system-font)');
});
