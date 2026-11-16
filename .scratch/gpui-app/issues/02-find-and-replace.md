# 02: Find and replace in the Note

**What to build:** ⌘F opens a find bar over the editor with the selection (or the word at the cursor) as the query: next (↩, ⌘G), previous (⇧↩, ⇧⌘G), all (selects every match), match case, regular expression, whole word, and a replace field with replace and replace all. Every match is highlighted; the current one is selected and scrolled into view. Esc closes the bar and returns to the text.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] `core::editor::find`: the matches of a query in a text, plain or regex, case-sensitive or not, whole words or not; an invalid regex finds nothing (the field shows it)
- [ ] Matches highlighted as a background, through the per-line styles, recomputed when the text or the query changes
- [ ] Replace replaces the current match and goes to the next; replace all is one undo step; a regex replace can use `$1`
- [ ] Tests: matching rules without GPUI; UI (⌘F, typing, ⌘G and ⇧⌘G moving and wrapping, replace all undone in one step, Esc)
