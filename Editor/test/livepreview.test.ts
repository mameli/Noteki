import { strict as assert } from "node:assert";
import { test } from "node:test";
import { syntaxTree } from "@codemirror/language";
import { renderedCode } from "../src/livepreview";
import { shown, stateOf } from "./helpers";

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
  assert.equal(shown(code), "\n**x** [[y]]\n\n**not bold**\n![[Pasted image.png]] ![alt](a.png)\n");
});

test("a code block hides its fences, its language kept, until the cursor is in it", () => {
  const block = "```swift\nlet x = 1\n```";
  assert.equal(shown(`${block}\n\n‸`), "swift\nlet x = 1\n\n\n");
  assert.equal(shown(`\`\`\`swift\nlet x‸ = 1\n\`\`\`\n`), `${block}\n`);
});

test("a code block is drawn line by line, with its language", () => {
  const state = stateOf("Text\n\n```swift\nlet x = 1\n```\n\n    indented\n‸");
  assert.deepEqual(renderedCode(state), [
    { lines: [6, 15, 25], info: { from: 9, to: 14 } },
    { lines: [30], info: null },
  ]);
});

test("code is coloured in its language, and a language not known gets the plain colouring", () => {
  // The innermost node at the start of the code's first line: CodeText when nothing parsed it.
  const nested = (text: string) => syntaxTree(stateOf(text)).resolveInner(text.indexOf("\n") + 1, 1);
  assert.equal(nested("```ts\nconst x = 1\n```\n").name === "CodeText", false);
  assert.equal(nested("```elixir\ndefmodule X do\n```\n").name === "CodeText", false);
  assert.equal(nested("```\nplain text\n```\n").name, "CodeText");
});

test("moving the cursor never changes what is shown on the lines above it", () => {
  const text = "# Title\n**bold** and *it*\n[link](u) [[Note|alias]]\n**across\nlines**\n`code` here\n%% takku:start %%\n%%\na comment\n\nacross lines\n%%\n==mark== text %%x%%\n## End";
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

test("a highlight shows without its ==, which appear with the cursor in it", () => {
  assert.equal(shown("==marked== and **b==c==d**\n‸"), "marked and bcd\n");
  assert.equal(shown("==mar‸ked== and ==other=="), "==marked== and other");
  // Not a highlight: spaces inside its marks, three = signs, a setext underline.
  assert.equal(shown("a == b == c, ===x===\nTitle\n==\n‸"), "a == b == c, ===x===\nTitle\n==\n");
});

test("a Comment shows faded, its %% hidden, and as written with the cursor in it", () => {
  assert.equal(shown("Before %%note%% after\n%% takku:start %%\nText\n%% takku:end %%\n‸"), "Before note after\n takku:start \nText\n takku:end \n");
  assert.equal(shown("a %%no‸te%% b"), "a %%note%% b");
  // Not closed on its paragraph: no Comment.
  assert.equal(shown("50%% of it\n‸"), "50%% of it\n");
});

test("a Comment across lines, blank lines and blocks included, shows its %% from the cursor's line down", () => {
  const comment = "%%\n- item\n\n# Not a heading\n%%\nafter";
  assert.equal(shown(comment + "‸"), "\n- item\n\n# Not a heading\n\nafter");
  assert.equal(shown("%%\n- item\n\n# Not a ‸heading\n%%\nafter"), "\n- item\n\n# Not a heading\n%%\nafter");
  assert.equal(shown("‸" + comment), comment);
  // Inside a paragraph too, in a quote or a list keeping their marks.
  assert.equal(shown("text %%one\ntwo%% end\n‸"), "text one\ntwo end\n");
  assert.equal(shown("> a %%x\n> y%% b\n‸"), "> a x\n> y b\n");
});

test("a Comment in code is code", () => {
  assert.equal(shown("`%%x%%`\n```\n%%\n```\n‸"), "%%x%%\n\n%%\n\n");
});

test("an empty Alias shows the name; in a table the escaped pipe is the Alias's", () => {
  assert.equal(shown("[[Note|]] and [[Note\\|alias]]\n‸"), "Note and alias\n");
});
