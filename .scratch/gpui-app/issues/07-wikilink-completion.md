# 07: `[[` completion while typing

**What to build:** Typing `[[` opens a list under the caret of the Vault's Notes, fuzzy on the name, the most recently opened first among equals (`VaultIndex::link_completions`, already used by the ⌘K picker). Typing filters it; ↑ ↓ choose; ↩ or Tab writes the name (the shortest unique path when ambiguous) and `]]`; Esc closes it. Inside a link already closed, it replaces the name and keeps the Alias or heading. Not in code. SPEC › Wikilinks.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Opens on `[[`, follows the typed name, closes on Esc, on a move out of the link, on `]]`
- [ ] The list at the caret (the element knows where it is)
- [ ] Choosing writes the link as one undo step
- [ ] Tests: the query under the caret and what choosing writes, without GPUI; UI (type `[[Rea`, ↩ gives `[[Reading list]]`)
