# 07: Merge and Conflict

**What to build:** When a Note changes on disk while the user has unsaved edits, Noteki does a three-way Merge with the last saved or loaded text as base and saves the result. When the two sides touch the same lines, it keeps the user's text, stops saving that Note and shows a banner with *Keep mine*, *Take theirs* and the differences.

**Blocked by:** 06 (Reload on External change)

**Status:** ready-for-agent

- [ ] Edits in different parts of the Note are merged and saved, nothing lost
- [ ] Edits to the same lines raise a Conflict: the user's text stays, the file on disk is not overwritten
- [ ] *Keep mine* saves the user's text; *Take theirs* loads the file's
- [ ] *Show differences* shows both versions
- [ ] Tested at Seam 1, including a replay of Takku's Processing writes while the user keeps typing in Personal notes
