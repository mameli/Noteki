# 11: Small editor gaps

**What to build:** ⌘K makes `[text](url)` of the selection, the URL from the clipboard if it holds one (SPEC › Format; the Wikilink picker moves to ⇧⌘K); ⌘/ wraps the selection, or the line, in `%%`; a table cell cut at its column shows an ellipsis; an indented code block (four spaces, not under a list item) is a code box; inside `$…$` and `$$…$$` nothing is taken as Markdown (math stays as written: `\,` and `\\` keep their backslash).

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Each with a unit test; UI for ⌘K and ⌘/
