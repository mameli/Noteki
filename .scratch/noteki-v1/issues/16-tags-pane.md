# 16: Tags pane

**What to build:** Tags are read from the `tags` Property (list or single value, with or without `#`) and from `#tag` in text outside code. The Tags pane shows them as a tree of nested Tags with counts; choosing a Tag lists its Notes, those of its sub-Tags included.

**Blocked by:** 11 (Vault index and clickable Wikilinks)

**Status:** ready-for-agent

- [x] Tags from Frontmatter and from text are both recognised; `#` inside code is not
- [x] `client/axo` appears under `client`; choosing `client` lists both
- [x] Each Tag shows its Note count
- [x] Tested at Seam 1

## Comments

Done. The left sidebar has a Files / Tags control at its top, remembered in the preferences. The Tags pane shows the Tags as a tree with the number of Notes, each counted once, parents included. Choosing a Tag lists its Notes, the Notes of its sub-Tags included, under the tree; clicking one opens it.

Tags are read from the `tags` Property (block list, flow list, one value, or several separated by commas or spaces; quotes and `#` dropped; a trailing YAML comment not read) and from `#tag` in the text, after the start of a line or a space and starting with a letter, outside inline and fenced code, outside the Frontmatter, and not escaped. Seam 1 tests cover each of these, nesting, counts, listing the Notes of a Tag, case, rereading and removing Notes, CRLF, a byte order mark and digits.

Decisions to know:
- **Tags are read when the Tags pane is first shown**, not at launch, as the Backlinks are. Reading 5,000 Notes takes about 2 s, and Noteki's process grows 1 to 5 MB. Until then "Reading the Tags of your Notes…".
- **`#Client` and `#client` are one Tag**, shown as most Notes write it, the lowercase form between equals. The SPEC didn't say; Obsidian does the same.
- **Only the `tags` key** is read in the Frontmatter, not `tag`, as the SPEC says. Say if the Vault has Notes with `tag:`.
- **`(#tag)` and `word#tag` are not Tags**, and a Tag of only digits (`#2024`) is not one. The SPEC's rule is "starts with a letter"; Obsidian's differs for `#1984a`.
- Not excluded as code, as for Backlinks: indented code blocks, fences nested in a list, `%%comments%%` and HTML.
- Backlinks and Tags now share the way the Notes are read, the Markdown rules for code, and one controller in the app (`ScannedNotes`). The Backlinks tests were not changed, and the app was rechecked with the sidebar shown.

Fixed after review: a comment after a value (`alpha # old`) dropped the Tag, `#a//b` was dropped instead of read as `a`, `²` counted as a digit, a blank line ended a block list, a byte order mark hid the Frontmatter, `tags: null` was a Tag, a Tag chosen and then removed left an empty list, "No Tags" flashed before the first read, `tree()` was quadratic in the number of Tags, and the Notes list used a table (memory).

Verified in the app, without taking the keyboard: the Tags tree with counts, nested Tags, the list of Notes, a Tag written to a file on disk appearing in the pane, and a click on a Note opening it. In code (`` `#code` ``) is not a Tag.
