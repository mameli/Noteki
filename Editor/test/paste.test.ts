import { strict as assert } from "node:assert";
import { test } from "node:test";
import { markdownOf } from "../src/paste";

test("formatted text is pasted as Markdown: headings, emphasis, links, lists, quotes, code", () => {
  const html =
    '<h2>Plan</h2><p>Some <b>bold</b>, <i>italic</i> and <a href="https://x.org">a link</a>.</p>' +
    "<ul><li>one</li><li>two</li></ul><ol><li>first</li><li>second</li></ol><blockquote><p>quoted</p></blockquote>" +
    "<p>Run <code>make</code>:</p><pre><code>make all\n</code></pre>";
  assert.equal(
    markdownOf(html, "Plan Some bold, italic and a link."),
    "## Plan\n\nSome **bold**, *italic* and [a link](https://x.org).\n\n- one\n- two\n\n1. first\n2. second\n\n> quoted\n\nRun `make`:\n\n```\nmake all\n```",
  );
});

test("a table with a header becomes a Markdown table", () => {
  const html = "<table><thead><tr><th>Name</th><th>Qty</th></tr></thead><tbody><tr><td>apple</td><td>12</td></tr></tbody></table>";
  assert.equal(markdownOf(html, "Name\tQty\napple\t12"), "| Name | Qty |\n| --- | --- |\n| apple | 12 |");
});

test("Google Docs' wrapper is no bold", () => {
  const html = '<meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-1"><p>Just <span>text</span> and <b>this</b></p></b>';
  assert.equal(markdownOf(html, "Just text and this"), "Just text and **this**");
});

test("text whose HTML is only its look stays plain: an editor's code, a plain paragraph", () => {
  const vscode = '<meta charset="utf-8"><div style="color: #d4d4d4;white-space: pre;"><div><span style="color: #569cd6;">let</span> x = 1</div></div>';
  assert.equal(markdownOf(vscode, "let x = 1"), null);
  assert.equal(markdownOf("<p>Only <span style='color:red'>words</span></p>", "Only words"), null);
});
