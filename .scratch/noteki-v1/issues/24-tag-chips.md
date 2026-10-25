# 24: Tag chips

**What to build:** The `tags` row of the Properties table shows Tags as chips, with a field to add one, completed from the Tags of the Vault, and a × to remove one. Noteki writes them back only in the `tags` Property, as a YAML list.

**Blocked by:** 23 (Properties table), 16 (Tags pane)

**Status:** ready-for-agent

- [x] Tags show as chips
- [x] Adding completes from the Vault's Tags and writes a YAML list
- [x] × removes the Tag from the Property
- [x] Tags in the text are never touched
- [x] Tested at Seam 1 for the Frontmatter written

## Comments

Done.
- **Chips** are drawn in place in the Properties table, one per line or side by side as the Tags are written; `tags: a, b` is a chip each.
  - A × removes one; a + after the last opens a field, which offers the Vault's Tags (most Notes first, those already there left out). Return, or choosing a Tag from the list, adds it.
  - Without a `tags` Property the + is on the table's top line, and the Property is added at the end of the Frontmatter.
- **Writing, in NotekiCore** (`Frontmatter.editingTags`): only the `tags` Property's lines change, rewritten as a list one item per line without quotes or `#` (`tags: []` once empty). The Note's line ends are kept. The editor asks (`addTag`, `removeTag`), and the app answers with the Frontmatter to put in place. The editor applies it as an ordinary edit (undone with ⌘Z, saved and merged as typing), only while its Frontmatter is still the text the answer was written from. Tags in the text are never sent, let alone touched.
- **Nothing changes**, and the YAML is left to be edited by hand, for:
  - a Tag already there in any case, one that is no Tag, or `null`/`true`/`false`;
  - YAML Noteki cannot read;
  - a `tags` Property with a comment among its lines, or with a value that needs its quotes.
- **The Vault's Tags** for the field come from NotekiCore's `TagMap.completions()`: every level as most Notes write it, those most Notes have first. They are read the first time they are asked for, as for the Tags pane (ADR 0006 updated).

Tests: Seam 1 (adding, removing, rewriting each way Tags are written, no `tags` Property, what is never rewritten, line ends, the completions, the bridge's JSON) and Seam 2 (chips, the field's place, a text of several Tags, an item with a space).

Not done, in the SPEC: a Note without Frontmatter has no field (its Tags are written as text); the Tags pane still reads `tags: alpha #beta` as two Tags where the Properties table, as YAML does, reads `#beta` as a comment, so such a Property is left alone.

Fixed after review:
- Tags in quotes for what they hold (`"a #b"`, `"x: y"`) would have been written back without them, losing or breaking them;
- `null`, `true` or `false` would have been written bare;
- a comment on the Tags' lines would have been lost;
- an item with a space was split into chips whose × did nothing;
- choosing a Tag from the list needed Return as well;
- the completions moved into NotekiCore with a test;
- pending requests are dropped on switching Vault.

Verified on the editor page in the browser: chips, the + field and its list without the Tags already there, a Tag added from Return and from the list, a chip removed, every line still 24 px. Verified in the app, launched in the background and driven through accessibility while Claude stayed in front: the chips, their × and the + read as buttons. Pressing the × of `meeting` made NotekiCore write `tags:` then `  - client/axo` to the file on disk, with the `#inline` Tag in the text untouched. Not tried in the app: adding a Tag, since Return needs the keyboard and Noteki cannot take it without being brought forward; the list of Tags offered in WebKit's own popup; the colours.
