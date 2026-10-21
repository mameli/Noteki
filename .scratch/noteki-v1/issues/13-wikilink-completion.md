# 13: Wikilink completion

**What to build:** Typing `[[` opens a list of Notes, fuzzy on the name, the most recently opened first. Choosing one writes its name, or the shortest unique path when the name is ambiguous, and closes the brackets.

**Blocked by:** 11 (Vault index and clickable Wikilinks)

**Status:** ready-for-agent

- [ ] `[[` opens the list; typing narrows it fuzzily
- [ ] Recently opened Notes come first
- [ ] An ambiguous name is written as the shortest unique path
- [ ] Tested at Seam 1 for the candidates and the text inserted
