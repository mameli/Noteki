# 23: Properties table

**What to build:** The Frontmatter is shown as a two-column table of Properties above the text, lists one item per line. Clicking it shows the YAML for editing; leaving renders it again. Frontmatter that does not parse is shown as source with an error mark and never rewritten.

**Blocked by:** 19 (Live preview of inline syntax)

**Status:** ready-for-agent

- [x] Properties render as a table; lists one item per line
- [x] Clicking the table shows the YAML; leaving renders it
- [x] Unparseable YAML stays source with an error mark
- [x] Tested at Seam 1 (parsing) and Seam 2 (what is shown)

## Comments

Done.
- **Reading, in NotekiCore** ([ADR 0009](../../../docs/adr/0009-properties-from-a-yaml-subset.md)): the YAML Obsidian and Takku write, by hand, with where every key, value and item is (a line and its UTF-16 columns). That is `key: value` at the margin, plain or in quotes; lists one item per line or in brackets; comments and blank lines. Anything else is YAML Noteki cannot read. All 159 Frontmatters of the user's Vault read (803 Properties).
- **Asking:** the editor asks the app (`readFrontmatter`) whenever its Frontmatter changes, and draws the table only while the Frontmatter is still the text the answer was about. Until then it is written as it is, at the same height. The app reads it in the editor's controller: it needs nothing of the Vault, and a Loose file has Properties too.
- **Drawing, in place, as tables are** (ADR 0008): every YAML line stays one line, never wrapped.
  - The `---` lines are the table's top ("Properties") and bottom; a key is a column as wide as the widest key.
  - A value shows without its quotes or comment, cut with an ellipsis when too long.
  - A list's items are pills, one per line under their key, or side by side when written `[a, b]`.
  - A comment on a line of its own is an empty row.
  - Clicking puts the cursor where it was in the text, and the YAML comes back on the same lines, moved sideways.
- **Unreadable YAML** stays as written, with a red wavy line where reading stopped. Nothing in this ticket writes the Frontmatter.
- **Opening:** a Note opens with the cursor after its Frontmatter, so the table shows. A Takku Meeting note then opens with `%% takku:start %%` shown, as the cursor is on that Comment's line.

Tests: Seam 1 (values, quotes, comments, lists, what cannot be read and where, UTF-16 columns and CRLF lines, the bridge's JSON) and Seam 2 (the table, comments, the cursor in the YAML, answers about another text, unreadable YAML, where a Note opens).

Not done, in the SPEC and ADR 0009:
- a list in brackets shows its items side by side, not one per line;
- `\"` and `''` in quotes are shown as written;
- valid YAML outside the subset is marked as unreadable;
- a Note of nothing but its Frontmatter opens on the YAML;
- the Tags pane still reads the `tags` Property with its own, looser reader, until ticket 24 writes Tags through this one.

Fixed after review:
- `a: - b` and `a: [x: y]` were read as a text and an item;
- an item continued on the next line was read as two;
- an empty key was accepted;
- a comment after a tab was part of the value;
- `[x, ]` was unreadable;
- comment lines showed inside the table;
- the Live preview did not redraw when the app answered (found on the editor page).

Verified on the editor page in the browser: a Takku Frontmatter drawn as a table, every line 24 px drawn and written, a click showing the YAML, an edit read again and drawn on leaving, the unreadable mark. Verified in the app, launched in the background and driven through accessibility while Claude stayed in front, with NotekiCore reading: the table's rows (keys, values, pills, the items under `participants`) and a broken Frontmatter left as written. Not looked at in the app window: the colours.
