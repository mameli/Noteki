# 11: Vault index and clickable Wikilinks

**What to build:** Noteki indexes the Vault in memory at launch and keeps it current from the change entry. `[[name]]` and `[[name|alias]]` in a Note are links: a click opens the Note they point to. Resolution follows Obsidian, including its rule for duplicate names, checked by hand in Obsidian on the fixture Vault first.

**Blocked by:** 02 (Excluded folders), 04 (Write and save a Note)

**Status:** ready-for-agent

- [x] The window is usable while indexing runs
- [x] Clicking a Wikilink opens its Note; an Alias shows its text but opens the Note
- [x] A name with `/` resolves as a path from the root; otherwise by file name anywhere
- [x] Duplicate names resolve as Obsidian does, recorded as tests after checking Obsidian by hand
- [x] Creating, changing or deleting a Note updates the index without a relaunch
- [x] Tested at Seam 1

## Comments

Done. The index holds the Vault's file names, rebuilt from every Files listing, including after changes on disk. Listing 5,000 Notes takes 0.17 s off the main thread, and the window stays usable meanwhile.

The duplicate-name rule was read from Obsidian 1.14.4's own code (`MetadataCache.getLinkpathDest`) rather than checked by hand, and reproduced with its quirks:
- text prefixes, so "Meet" also matches "Meetings/";
- a path also matches the end of a longer one;
- `[[#heading]]` leads to the Note itself.

The review compared the two line by line. Ties in path length keep Files' order. Only files Noteki lists can be found.

The editor parses `[[…]]` and `![[…]]` on one line: an escaped `\|` in tables, no closing past a code span, and `[[[a]]` is not a link. A click follows a link on release, without a drag, Control or an input method composing, and only over the text itself. With the cursor already in the link, the click places it.

Verified in the app with real mouse clicks: alias link, path link, Embed, table link, an unresolved link that beeps, a drag starting on a link, and a click beside a line.

Not done: revealing a Note does not expand its collapsed folders in Files.
