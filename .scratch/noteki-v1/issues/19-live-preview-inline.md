# 19: Live preview of inline syntax

**What to build:** The editor shows Notes formatted: Headings, emphasis, inline code, links and Wikilinks render, and their syntax appears only when the cursor is inside that element. ⌘E switches to Source mode. Notes above 5 MB open in Source mode. This sets up the Seam 2 tests.

**Blocked by:** 11 (Vault index and clickable Wikilinks)

**Status:** ready-for-agent

- [ ] Bold, italic, inline code, links, Wikilinks and Headings render with their marks hidden
- [ ] The marks of an element appear when the cursor enters it, not on the whole line
- [ ] Showing or hiding marks never changes the height of lines above the cursor
- [ ] ⌘E toggles Source mode
- [ ] Tested at Seam 2: given a text and a cursor, what is hidden
