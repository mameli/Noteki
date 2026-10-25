import { strict as assert } from "node:assert";
import { test } from "node:test";
import { shown } from "./helpers";

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

test("a Comment is hidden, its line kept, and shown with the cursor in it", () => {
  assert.equal(shown("Before %%hidden%% after\n%% takku:start %%\nText\n%% takku:end %%\n‸"), "Before  after\n\nText\n\n");
  assert.equal(shown("a %%hid‸den%% b"), "a %%hidden%% b");
  // Not closed on its paragraph: no Comment.
  assert.equal(shown("50%% of it\n‸"), "50%% of it\n");
});

test("a Comment across lines, blank lines and blocks included, shows from the cursor's line down", () => {
  const comment = "%%\n- item\n\n# Not a heading\n%%\nafter";
  assert.equal(shown(comment + "‸"), "\n\n\n\n\nafter");
  assert.equal(shown("%%\n- item\n\n# Not a ‸heading\n%%\nafter"), "\n\n\n# Not a heading\n%%\nafter");
  assert.equal(shown("‸" + comment), comment);
  // Inside a paragraph too, and from a line of its own right under one.
  assert.equal(shown("text %%one\ntwo%% end\n‸"), "text \n end\n");
  assert.equal(shown("Text\n%%\nA comment\n\nacross\n%%\nAfter‸"), "Text\n\n\n\n\n\nAfter");
  // Right under a list too.
  assert.equal(shown("- item\n%%\nhidden\n\nmore\n%%\nafter‸"), "- item\n\n\n\n\n\nafter");
  // A paragraph's Comment across lines in a quote or a list keeps their marks.
  assert.equal(shown("> a %%x\n> y%% b\n\n- a %%x\n  y%% b\n‸"), "> a \n>  b\n\n- a \n   b\n");
  // That line may close a Comment the paragraph opened.
  assert.equal(shown("text %%one\n%% end\n\nmore\n‸"), "text \n end\n\nmore\n");
});

test("a Comment in code is code", () => {
  assert.equal(shown("`%%x%%`\n```\n%%\n```\n‸"), "%%x%%\n```\n%%\n```\n");
});

test("an empty Alias shows the name; in a table the escaped pipe is the Alias's", () => {
  assert.equal(shown("[[Note|]] and [[Note\\|alias]]\n‸"), "Note and alias\n");
});
