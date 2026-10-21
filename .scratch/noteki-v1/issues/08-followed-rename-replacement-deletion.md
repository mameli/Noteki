# 08: Followed rename, replacement and deletion

**What to build:** An open Note renamed or moved by another app stays open under its new name. A Note replaced by an atomic write (new file at the same path) is treated as changed. A Note deleted by another app stays open with its text and a banner offering to save it again.

**Blocked by:** 06 (Reload on External change)

**Status:** ready-for-agent

- [x] Renaming an open Note in the Finder updates its title and later saves go to the new path
- [x] Takku's rename after Processing keeps the Meeting note open
- [x] An atomic replacement reloads (or Merges) instead of looking like a deletion
- [x] Deleting an open Note shows a banner; *Save again* recreates it with the text on screen
- [x] Tested at Seam 1 for each case

## Comments

Done. The open Note follows its file by identity (device and inode), refreshed after Noteki's own atomic saves. When a new file is put in place and renamed in the same report, so the identity is new too, it falls back to the text. It follows the one changed Note that keeps most of the lines Noteki knew, if exactly one does.

Covered at Seam 1, 49 tests in all:
- a Finder rename, also reported in two parts;
- Takku's in-place write then rename;
- an atomic replacement, also with the same bytes, then a rename;
- an atomic write and a rename in one report;
- a swap (A→C, then B→A), where the Note follows C and B is not taken for it;
- a case-only rename;
- a deletion keeping the text, with Save Again ending any Conflict;
- a deleted Note written again at its path;
- a rename while edits are unsaved, where a save in between waits for the event instead of failing.

Found in the app: events for a file that is gone did not match a Vault reached through a symbolic link (/tmp, /var). `Vault.changedPaths` now resolves real paths.

Verified in the app on a demo Vault: Takku-style write and rename, typing after it, atomic replacement and rename, a case-only rename, deletion with the banner, Save Again, and leaving a Deleted Note, which asks first.

Not handled: inode reuse on HFS+, exFAT or SMB volumes (APFS does not reuse them quickly), hard links, and a Note that is a symbolic link to a file outside the Vault.
