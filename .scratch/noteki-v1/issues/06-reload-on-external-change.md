# 06: Reload on External change

**What to build:** When a Note is changed on disk by another app and Noteki has nothing unsaved for it, the open Note reloads silently, the cursor kept at the same place in the text. The app's FSEvents watcher only tells the Vault module which paths changed.

**Blocked by:** 04 (Write and save a Note)

**Status:** ready-for-agent

- [ ] Changing an open Note in another editor shows the new text within a second
- [ ] The cursor stays at the same place in the text after the reload
- [ ] Noteki's own saves do not trigger a reload
- [ ] Tested at Seam 1 by writing the file and calling the Vault's change entry, without FSEvents
