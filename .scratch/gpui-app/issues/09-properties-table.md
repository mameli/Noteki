# 09: The Properties table over the Frontmatter

**What to build:** SPEC › Frontmatter and ADR 0009: the Frontmatter drawn in place as a two-column table, "Properties" at its top, keys in the first column, values as written, lists one item per line; the cursor entering it shows the YAML. The `tags` row as chips, a × to remove one and a + to add one (completed from the Vault's Tags); Noteki writes `tags` back as a list, an undoable edit. YAML Noteki cannot read stays as source. `core::frontmatter` already reads the YAML subset.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Table drawn with line decorations, lines never wrapped, the same height in table and source
- [ ] Tag chips, × and +, writing back only the `tags` Property
- [ ] Long values cut, not wrapped character by character (the user's feedback screenshot)
- [ ] Tests: the table's rows and the `tags` rewrite without GPUI; UI (a × removes a Tag, ⌘Z brings it back)
