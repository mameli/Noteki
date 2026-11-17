# 09: The Properties table over the Frontmatter

**What to build:** SPEC › Frontmatter and ADR 0009: the Frontmatter drawn in place as a two-column table, "Properties" at its top, keys in the first column, values as written, lists one item per line; the cursor entering it shows the YAML. The `tags` row as chips, a × to remove one and a + to add one (completed from the Vault's Tags); Noteki writes `tags` back as a list, an undoable edit. YAML Noteki cannot read stays as source. `core::frontmatter` already reads the YAML subset.

**Blocked by:** none

**Status:** ready-for-agent

- [x] Table drawn with line decorations, lines never wrapped, the same height in table and source
- [x] Tag chips (done), × and + writing back the `tags` Property (not yet: see the comments)
- [ ] Long values cut, not wrapped character by character (the user's feedback screenshot)
- [x] Tests: the table's rows without GPUI; UI (the table, the YAML with the cursor in it, a Note opened after its Frontmatter)

## Comments

Mostly done (2026-11-17), checked in the real app on a Frontmatter like the user's feedback screenshot (a long title and description, a Wikilink author, a list, `tags: [a, b]`).

- `core::editor::properties::property_lines`: when `core::frontmatter` can read the YAML, each Frontmatter line as a row: the top (`---`), `key: value`, a list's `- item` under its key, the bottom (`---`); a `tags` row's Tags, and what of a value is not shown (its quotes; a list's `[`, `]` and commas). YAML Noteki cannot read stays as source (SPEC) — a plain value holding `: ` is such YAML, as in YAML itself.
- Drawn with the table cells of ticket 12: two columns, the key column 9 em, values cut at the edge (no more one-letter-per-line wrapping, the user's feedback), a box like a code block's with "Properties" in place of the top `---`. Tags are chips (link colour on a tint). Every row keeps the text's line height, so showing the YAML moves nothing.
- With the cursor on any Frontmatter line, the whole Frontmatter shows as YAML (the cursor pass restyles its lines); leaving it brings the table back.
- A Note now opens with the cursor at the start of its text, after the Frontmatter (SPEC › Window), so the table shows at once.
- **Not done yet**: the chips' × and + (removing and adding a Tag by writing the `tags` Property back as a list). Until then Tags are edited in the YAML, by clicking into the table.
- Tests: rows, items, Tags, hidden quotes and list marks, unreadable YAML; UI (values in one column, YAML with the cursor in it at the same heights, a Note opened after its Frontmatter). New mutant: the table staying with the cursor in it. Killed.
