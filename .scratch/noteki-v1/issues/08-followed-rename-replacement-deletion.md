# 08: Followed rename, replacement and deletion

**What to build:** An open Note renamed or moved by another app stays open under its new name. A Note replaced by an atomic write (new file at the same path) is treated as changed. A Note deleted by another app stays open with its text and a banner offering to save it again.

**Blocked by:** 06 (Reload on External change)

**Status:** ready-for-agent

- [ ] Renaming an open Note in the Finder updates its title and later saves go to the new path
- [ ] Takku's rename after Processing keeps the Meeting note open
- [ ] An atomic replacement reloads (or Merges) instead of looking like a deletion
- [ ] Deleting an open Note shows a banner; *Save again* recreates it with the text on screen
- [ ] Tested at Seam 1 for each case
