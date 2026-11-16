# 02: Find and replace in the Note

**What to build:** ⌘F opens a find bar over the editor with the selection (or the word at the cursor) as the query: next (↩, ⌘G), previous (⇧↩, ⇧⌘G), all (selects every match), match case, regular expression, whole word, and a replace field with replace and replace all. Every match is highlighted; the current one is selected and scrolled into view. Esc closes the bar and returns to the text.

**Blocked by:** 01

**Status:** ready-for-agent

- [x] `core::editor::find`: the matches of a query in a text, plain or regex, case-sensitive or not, whole words or not; an invalid regex finds nothing (the field shows it)
- [x] Matches highlighted as a background (painted as rectangles under the selection, not through the line styles: see the comments), found again only when the text or the query changes
- [x] Replace replaces the current match and goes to the next; replace all is one undo step; a regex replace can use `$1`
- [x] Tests: matching rules without GPUI; UI (⌘F, typing, ⌘G and ⇧⌘G moving and wrapping, replace all undone in one step, Esc)

## Comments

Done (2026-11-17), except the "all" button, which needs several selections: it comes with ticket 03.

- `core::editor::find`: a query (text, match case, regex, whole word) compiled to one `regex` (plain text escaped, whole words wrapped in `\b`, case-insensitive unless asked); every non-empty match; an invalid regex is an error the bar shows. Replace fills in `$1`... for a regex only; replace all is one replacement of the whole text through `replace_all` (one undo step).
- `src/editor/find_bar.rs`: the bar is rendered above the editor, outside its key context (typing in it is not editing the Note), with GPUI Kit's `Input`s: Find and Replace fields, Aa / W / .* toggles, Previous, Next, a status ("2 of 5", "No results", "Invalid regex"), Replace, Replace all, Done. ⌘F seeds it with the selection or the word at the cursor and selects the match there; ↩ / ⌘G next, ⇧↩ / ⇧⌘G previous, both wrapping; Esc closes it and gives the text the focus back.
- Matches are found again only when the text's revision or the query changes; they are painted as rectangles (the find highlight colour) under the selection. Simpler than going through the line styles, and as cheap.
- Tests: matching rules (case, whole word, regex, invalid regex, no empty matches, replacement with `$2 $1`, a plain `$`); UI (⌘F seeds and selects, ⌘G and ⇧⌘G wrap, whole word, Esc, replace all undone in one step). New mutant: whole word finding a word inside another. Killed.
