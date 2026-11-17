# 11: Small editor gaps

**What to build:** ⌘K makes `[text](url)` of the selection, the URL from the clipboard if it holds one (SPEC › Format; the Wikilink picker moves to ⇧⌘K); ⌘/ wraps the selection, or the line, in `%%`; a table cell cut at its column shows an ellipsis; an indented code block (four spaces, not under a list item) is a code box; inside `$…$` and `$$…$$` nothing is taken as Markdown (math stays as written: `\,` and `\\` keep their backslash).

**Blocked by:** none

**Status:** ready-for-agent

- [x] Each with a unit test; UI for ⌘K and ⌘/

## Comments

Done (2026-11-17), checked in the real app with a sample (an indented block, a table wider than the text, inline and block math).

- **⌘K** (`format::make_link`): `[text](url)` of the selection or the word at the cursor, the URL from the clipboard when it holds one (a token with `://`), else the cursor between the parentheses, or the brackets without text. The ⌘K Wikilink picker moved to ⇧⌘K.
- **⌘/** (`format::toggle_comment`): `%%` around the selection, or the whole line without one; again, they go. Both one undo step.
- **Ellipsis**: a table cell wider than its column is cut where `…` still fits.
- **Indented code**: four spaces or a tab after a blank line, not a list item nor under one, is a code box (no Markdown inside, the code font).
- **Math as written**: `$x$` (SPEC's rule: no space after the first `$`, none before the last, no digit after it: `$5 and $10` is text) and `$$x$$` are tokens nothing is parsed in, drawn as code; a `$$` block across lines is its own block kind. `\,`, `\\` and `\pi` keep their backslashes.
- A mutant on the digit rule survived at first: the only test, `$5 and $10`, is already ruled out by the space before the second `$`. A case only the digit rule rules out (`$a$1`) now kills it.
- Tests: link and comment rules; indented code vs a list continuation; math tokens and block; UI (⌘K with a URL on the clipboard, ⌘/, two undos).
