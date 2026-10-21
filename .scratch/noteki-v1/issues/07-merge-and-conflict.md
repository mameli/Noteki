# 07: Merge and Conflict

**What to build:** When a Note changes on disk while the user has unsaved edits, Noteki does a three-way Merge with the last saved or loaded text as base and saves the result. When the two sides touch the same lines, it keeps the user's text, stops saving that Note and shows a banner with *Keep mine*, *Take theirs* and the differences.

**Blocked by:** 06 (Reload on External change)

**Status:** ready-for-agent

- [x] Edits in different parts of the Note are merged and saved, nothing lost
- [x] Edits to the same lines raise a Conflict: the user's text stays, the file on disk is not overwritten
- [x] Replaces the interim guard from ticket 04 (a save refused with `changedOnDisk` and an alert on leaving the Note)
- [x] *Keep mine* saves the user's text; *Take theirs* loads the file's
- [x] *Show differences* shows both versions
- [x] Tested at Seam 1, including a replay of Takku's Processing writes while the user keeps typing in Personal notes

## Comments

Done. The three-way Merge is line based, with the lines neither side changed as fixed points. Fuzzed by the review with about 200,000 cases: with unique lines no line is ever lost, duplicated or reordered. Repeated lines such as blank lines align ambiguously in about 0.7% of cases, as in diff3 and git. Edits to adjacent lines, with no unchanged line between, are a Conflict, and the glossary now says so.

Verified in the app on a demo Vault:
- Keys typed into Personal notes while another process wrote Takku-style frontmatter and a Summary ended up merged, on disk and in the editor.
- Same-line edits raised the banner. *Show Differences* puts both versions side by side.
- *Take Theirs* keeps the cursor where it was, and the next key adds to the file's version.
- *Keep Mine* writes the user's version.

Kept on purpose: leaving a Note with an open Conflict still asks (*Keep Editing*, *Discard My Edits*, *Replace With My Version*). Otherwise leaving would lose the user's version. *Keep Mine* has no Return shortcut, so pressing Return while writing cannot erase the other change.
