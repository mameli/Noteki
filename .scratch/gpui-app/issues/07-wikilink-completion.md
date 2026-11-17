# 07: `[[` completion while typing

**What to build:** Typing `[[` opens a list under the caret of the Vault's Notes, fuzzy on the name, the most recently opened first among equals (`VaultIndex::link_completions`, already used by the ⌘K picker). Typing filters it; ↑ ↓ choose; ↩ or Tab writes the name (the shortest unique path when ambiguous) and `]]`; Esc closes it. Inside a link already closed, it replaces the name and keeps the Alias or heading. Not in code. SPEC › Wikilinks.

**Blocked by:** none

**Status:** ready-for-agent

- [x] Opens on `[[`, follows the typed name, closes on Esc, on a move out of the link, on `]]`
- [x] The list at the caret (the element knows where it is)
- [x] Choosing writes the link as one undo step
- [x] Tests: the query under the caret and what choosing writes, without GPUI; UI (type `[[Rea`, ↩ gives `[[Reading list]]`)

## Comments

Done (2026-11-17).

- `core::editor::completion`: `link_query` finds the name the cursor is typing after a `[[` on its line (not past a `|` or `#`, not in a closed link, not in code or inline code), its range to replace up to the `|`, `#` or `]]` that ends it; `rank` filters the Vault's Notes (prefix, then part, then letters in order, each in the given order: the most recently opened first); `choose` writes the link (the shortest unique path, from `VaultIndex::link_completions`) and `]]` when the link was not closed, keeping an Alias or heading.
- The editor shows at most 8 Notes in a list anchored under the caret (GPUI's `anchored` + `deferred`), only while typing: it is tied to the `(revision, cursor)` of the last edit, so clicking or moving into a link does not open it. ↑ ↓ choose, ↩ or Tab write (one undo step), Esc closes it until the cursor leaves the link; a click on a Note writes it. The app gives the Notes through `LinkContext::completions`.
- Tab inserts a tab character otherwise (it is now bound in the editor).
- Tests: query, closed link with an Alias, writing, ranking; robustness (the query is a valid range right after a `[[`, before the cursor); UI (`[[Rea` + ↩ gives `[[Reading list]]`, the list closes, Esc closes it, Return is a new line again). New mutant: the link left open. Killed.
