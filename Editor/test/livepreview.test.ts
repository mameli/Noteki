import { strict as assert } from "node:assert";
import { test } from "node:test";
import { ensureSyntaxTree } from "@codemirror/language";
import { EditorSelection, EditorState } from "@codemirror/state";
import { noteLanguage } from "../src/language";
import { hiddenRanges } from "../src/livepreview";

/**
 * The text as Live preview shows it: `‸` is the cursor (or `‸…‸` a selection), and what is
 * hidden is left out.
 */
function shown(marked: string): string {
  const first = marked.indexOf("‸");
  const second = marked.indexOf("‸", first + 1);
  const text = marked.replaceAll("‸", "");
  const anchor = first === -1 ? text.length : first;
  const head = second === -1 ? anchor : second - 1;
  const state = EditorState.create({
    doc: text,
    selection: EditorSelection.single(anchor, head),
    extensions: [noteLanguage],
  });
  ensureSyntaxTree(state, state.doc.length, 5000);
  let result = "";
  let at = 0;
  for (const { from, to } of hiddenRanges(state)) {
    result += text.slice(at, from);
    at = to;
  }
  return result + text.slice(at);
}

test("bold, italic, inline code and strikethrough show without their marks", () => {
  assert.equal(shown("**bold**, *it*, _also_, `code` and ~~gone~~\n‸"), "bold, it, also, code and gone\n");
});

test("the marks of the element the cursor is in appear, and only those", () => {
  assert.equal(shown("**bo‸ld** and *it*"), "**bold** and it");
  assert.equal(shown("**bold** and *i‸t*"), "bold and *it*");
  // At the edge of an element counts as inside: typing there extends it.
  assert.equal(shown("**bold**‸ and *it*"), "**bold** and it");
});

test("a selection shows the marks of the elements it touches, on the line where it ends", () => {
  assert.equal(shown("**bo‸ld** and *i‸t* and `c`"), "**bold** and *it* and c");
  // Selected from the first line down: the lines above the end keep their look.
  assert.equal(shown("**a‸a**\n*bb*\n`c‸c`"), "aa\nbb\n`cc`");
});

test("links show their text, Wikilinks their name or Alias", () => {
  assert.equal(shown("[Text](https://example.com \"title\") and <https://x.org>\n‸"), "Text and https://x.org\n");
  assert.equal(shown("[[Note]] and [[Folder/Note|the alias]] and [[Note#Part]]\n‸"), "Note and the alias and Note#Part\n");
  assert.equal(shown("[[Folder/Note|the al‸ias]]"), "[[Folder/Note|the alias]]");
});

test("Headings show without their #s, and with them when the cursor is on their line", () => {
  assert.equal(shown("# Title\n## Sub ##\ntext‸"), "Title\nSub\ntext");
  assert.equal(shown("# Ti‸tle\n## Sub"), "# Title\nSub");
});

test("an element across lines shows only the marks on the cursor's line", () => {
  assert.equal(shown("**bold\nmo‸re**"), "bold\nmore**");
  assert.equal(shown("**bo‸ld\nmore**"), "**bold\nmore");
});

test("code, Embeds and images are left as they are", () => {
  const code = "```\n**x** [[y]]\n```\n`**not bold**`\n![[Pasted image.png]] ![alt](a.png)\n‸";
  assert.equal(shown(code), "```\n**x** [[y]]\n```\n**not bold**\n![[Pasted image.png]] ![alt](a.png)\n");
});

test("moving the cursor never changes what is shown on the lines above it", () => {
  const text = "# Title\n**bold** and *it*\n[link](u) [[Note|alias]]\n**across\nlines**\n`code` here\n## End";
  const far = shown(text + "\n‸").split("\n");
  for (let position = 0; position <= text.length; position++) {
    const lines = shown(text.slice(0, position) + "‸" + text.slice(position)).split("\n");
    const cursorLine = text.slice(0, position).split("\n").length - 1;
    for (let line = 0; line < cursorLine; line++) {
      assert.equal(lines[line], far[line], `line ${line + 1} with the cursor at ${position}`);
    }
  }
});

test("a link whose title is on the next line is left as written", () => {
  assert.equal(shown('[a](u\n"t") and **b**\n‸'), '[a](u\n"t") and b\n');
});

test("an empty Alias shows the name; in a table the escaped pipe is the Alias's", () => {
  assert.equal(shown("[[Note|]] and [[Note\\|alias]]\n‸"), "Note and alias\n");
});
