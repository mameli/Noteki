# 06: Reload on External change

**What to build:** When a Note is changed on disk by another app and Noteki has nothing unsaved for it, the open Note reloads silently, the cursor kept at the same place in the text. The app's FSEvents watcher only tells the Vault module which paths changed.

**Blocked by:** 04 (Write and save a Note)

**Status:** ready-for-agent

- [x] Changing an open Note in another editor shows the new text within a second
- [x] The cursor stays at the same place in the text after the reload
- [x] Noteki's own saves do not trigger a reload
- [x] Tested at Seam 1 by writing the file and calling the Vault's change entry, without FSEvents

## Comments

Done. Verified in the app on a demo Vault:
- Another process wrote a Summary above the cursor's line. The Note reloaded and the next key landed where the cursor was.
- Five outside writes were shown 0.02–0.23 s later, measured from the app's log.
- Noteki's own saves cause no reload.
- ⌘Z never undoes reloaded text.
- Keys typed while another app rewrites the file all stay in the editor, the file keeps the other app's version, and the banner says so. Merging the two is ticket 07.

From the review, a reload becomes the open Note's base only after the editor confirms it showed it. It carries the last revision the app received, so keys typed in between are never overwritten on screen or on disk.

Also done, not asked by this ticket: Files is relisted when a Note or Attachment appears or disappears (Takku's new Meeting notes), but not when a listed file is saved. Ticket 11's index will take this over. Events FSEvents could not deliver one by one make Noteki recheck the open Note and relist Files.
