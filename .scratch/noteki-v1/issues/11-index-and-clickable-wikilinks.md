# 11: Vault index and clickable Wikilinks

**What to build:** Noteki indexes the Vault in memory at launch and keeps it current from the change entry. `[[name]]` and `[[name|alias]]` in a Note are links: a click opens the Note they point to. Resolution follows Obsidian, including its rule for duplicate names, checked by hand in Obsidian on the fixture Vault first.

**Blocked by:** 02 (Excluded folders), 04 (Write and save a Note)

**Status:** ready-for-agent

- [ ] The window is usable while indexing runs
- [ ] Clicking a Wikilink opens its Note; an Alias shows its text but opens the Note
- [ ] A name with `/` resolves as a path from the root; otherwise by file name anywhere
- [ ] Duplicate names resolve as Obsidian does, recorded as tests after checking Obsidian by hand
- [ ] Creating, changing or deleting a Note updates the index without a relaunch
- [ ] Tested at Seam 1
