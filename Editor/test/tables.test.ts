import { strict as assert } from "node:assert";
import { test } from "node:test";
import { renderedTables } from "../src/livepreview";
import { ensureSyntaxTree } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { noteLanguage } from "../src/language";
import { cursorInCells, fitColumns } from "../src/tables";
import { shown, stateOf as state } from "./helpers";

/** The tables found looking only between `from` and `to`, as for the part of a Note in view. */
function stateWithin(marked: string, from: number, to: number) {
  return { state: state(marked), from, to };
}

const table = "| Name | Qty |\n|:-----|----:|\n| apple | 12 |\n| **pear** | |";

test("a table away from the cursor shows its cells, its pipes and delimiter row hidden, every line kept", () => {
  // An empty cell keeps one space, to be drawn as the cell.
  assert.equal(shown(`Before\n\n${table}\n\nAfter‸`), "Before\n\nNameQty\n\napple12\npear \n\nAfter");
});

test("its columns have the width of their widest cell as shown, and the alignment of the delimiter row", () => {
  const [found] = renderedTables(state(`${table}\n\n‸`));
  assert.deepEqual(found.columns, [
    { width: 5, align: "left" },
    { width: 3, align: "right" },
  ]);
  assert.deepEqual(
    found.rows.map((row) => [row.kind, row.cells.map((cell) => cell.text)]),
    [
      ["header", ["Name", "Qty"]],
      ["rule", []],
      ["body", ["apple", "12"]],
      ["body", ["**pear**", ""]],
    ],
  );
});

test("a table the cursor is in stays drawn, the cell it is in edited, its marks shown", () => {
  const [found] = renderedTables(state("| Na‸me | Qty |\n|---|---|\n| a | 1 |"));
  assert.deepEqual(found.editing, [{ row: 0, column: 0 }]);
  assert.equal(shown("| Name | Qty |\n|---|---|\n| `a`‸ | 1 |"), "NameQty\n\n`a`1");
  assert.equal(renderedTables(state("| Name | Qty |\n|---|---|\n| a | 1 |\n\n‸"))[0].editing, undefined);
});

test("the cursor goes into a cell: off a hidden pipe, and off the delimiter row into the header", () => {
  const text = "| Name | Qty |\n|---|---|\n| a | 1 |";
  const start = EditorState.create({ doc: text, extensions: [noteLanguage, cursorInCells] });
  ensureSyntaxTree(start, text.length, 5000);
  const at = (pos: number) => start.update({ selection: { anchor: pos } }).state.selection.main.head;
  // The line's start, before its first pipe: the first cell's start.
  assert.equal(at(0), 2);
  // Between `Name` and the pipe after it: the end of `Name`.
  assert.equal(at(7), 6);
  // In the cell: where it was.
  assert.equal(at(4), 4);
  // On the delimiter row, after its second pipe: the end of the header's second cell.
  assert.equal(at(text.indexOf("|---|---|") + 6), 12);
});

test("rows without outer pipes, an empty cell, centred and default alignment", () => {
  const [found] = renderedTables(state("a | b | c\n:-:|---|---\n1 | | 3\n\n‸"));
  assert.deepEqual(found.columns.map((column) => column.align), ["center", "left", "left"]);
  assert.deepEqual(found.rows[2].cells.map((cell) => cell.text), ["1", "", "3"]);
});

test("a very wide column is capped", () => {
  const [found] = renderedTables(state(`| ${"x".repeat(100)} |\n|---|\n\n‸`));
  assert.equal(found.columns[0].width, 40);
});

test("an empty cell between two pipes keeps its column; an escaped pipe shows without its backslash", () => {
  const [found] = renderedTables(state("| a | b | c |\n|---|---|---|\n| 1 || 3 |\n| x \\| y | z | w |\n\n‸"));
  assert.deepEqual(found.rows[2].cells.map((cell) => cell.text), ["1", "", "3"]);
  assert.equal(shown("| a |\n|---|\n| x \\| y |\n\n‸"), "a\n\nx | y\n\n");
});

test("a table in a quote is drawn from its line's start; ⌘A leaves tables shown", () => {
  const [quoted] = renderedTables(state("> | a | b |\n> |---|---|\n> | 1 | 2 |\n\n‸"));
  assert.equal(quoted?.rows[0].lineStart, 0);
  assert.equal(renderedTables(state("‸before\n\n| a |\n|---|\n| 1 |\n\nafter‸")).length, 1);
});

test("a row's cells past the header's are not shown, as in GFM", () => {
  const [found] = renderedTables(state("| a |\n|---|\n| 1 | 2 |\n\n‸"));
  assert.equal(found.columns.length, 1);
  assert.equal(shown("| a |\n|---|\n| 1 | 2 |\n\n‸"), "a\n\n1\n\n");
});

test("a column is as wide over the whole table, whatever part of it is measured", () => {
  const within = stateWithin("| a |\n|---|\n| [[Note\\|alias]] |\n| xx |\n\n‸", 0, 6);
  const [found] = renderedTables(within.state, within.from, within.to);
  assert.equal(found.columns[0].width, 5);
});

test("a table wider than the text shrinks its widest columns only, to one width", () => {
  assert.deepEqual(fitColumns([5, 10, 30], 50), [5, 10, 30]);
  assert.deepEqual(fitColumns([5, 10, 30, 40], 45), [5, 10, 15, 15]);
  assert.deepEqual(fitColumns([20, 20], 30), [15, 15]);
  // Not laid out yet: as they are.
  assert.deepEqual(fitColumns([20, 20], 0), [20, 20]);
});
