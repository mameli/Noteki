import { strict as assert } from "node:assert";
import { test } from "node:test";
import { createDocument } from "@mixmark-io/domino";
import { setEmbeds } from "../src/embeds";
import { noteBody } from "../src/export";
import { stateOf } from "./helpers";

const parse = (html: string) => createDocument(html);

test("the Note is exported without its Frontmatter and Comments, Wikilinks as their name or Alias", async () => {
  const state = stateOf("---\ntags: a\n---\n# Title\n\nSee [[Folder/Note|the alias]] and [[Other]] %%hidden%% and ==this==.\n‸");
  const body = await noteBody(state, { parse });
  assert.equal(body.includes("tags"), false);
  assert.equal(body.includes("hidden"), false);
  assert.equal(body.includes("<h1>Title</h1>"), true);
  assert.equal(body.includes('<span class="wikilink">the alias</span>'), true);
  assert.equal(body.includes('<span class="wikilink">Other</span>'), true);
  assert.equal(body.includes("<mark>this</mark>"), true);
});

test("tasks, callouts, footnotes and images of the Vault are drawn", async () => {
  const text = "- [x] done\n- [ ] open\n\n> [!warning] Careful\n> Text\n\nA note[^1].\n\n[^1]: The note.\n\n![[photo.png|200]]\n‸";
  const state = stateOf(text);
  const withFiles = state.update({ effects: setEmbeds.of({ "photo.png|200": { path: "Media/photo.png", kind: "image" } }) }).state;
  const body = await noteBody(withFiles, { parse });
  assert.equal(body.includes('class="task done"'), true);
  assert.equal(body.includes('<div class="callout callout-warning"><div class="callout-title">Careful</div>'), true);
  assert.equal(body.includes('class="footnotes"'), true);
  assert.equal(body.includes('<img src="noteki-file://vault/Media/photo.png" alt="photo.png" width="200">'), true);
});

test("formulas without KaTeX stay as code; code is left as written", async () => {
  const body = await noteBody(stateOf("Inline $x^2$.\n\n```\n[[not a link]] ==no==\n```\n‸"), { parse });
  assert.equal(body.includes("<code>x^2</code>"), true);
  assert.equal(body.includes("[[not a link]] ==no=="), true);
});

test("lines one under the other stay apart, as Obsidian shows them", async () => {
  const body = await noteBody(stateOf("One line\nthe next\n‸"), { parse });
  assert.equal(body.includes("One line<br>\nthe next"), true);
});
