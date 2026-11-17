# 14: A second window

**What to build:** ⇧⌘N opens another window of the same Vault with its own Tabs (ADR 0010); a Note open in both is kept in step through the file, as with Takku. Each window's Tabs restored at launch. New folder moves from ⇧⌘N to the Files toolbar and the context menu (ticket 05).

**Blocked by:** none

**Status:** done

- [x] Two windows editing the same Note: typing in one shows in the other after save
- [x] Sessions per window

## Comments

- ⇧⌘N (and "New window" in the Command palette) opens another `NotekiApp` on the same Vault, in the first free slot. Each window has its own Tabs, documents and file watcher, so a Note open in two windows is kept in step through the file, merged as with Takku. New folder lost ⇧⌘N; it stays in the Files toolbar and the context menu.
- Sessions: slot 0 under the Vault's root, the others under `root#n`. Each window saves only its own session, keeping the other sessions on disk (`Settings::merged_for_save`). The first window reopens the others at launch. A window the user closes is forgotten; one closed by quitting comes back.
- Theme, appearance, text size, fonts, line length, Excluded folders and Daily note reach every window at once (`Settings::adopt_shared`). Sidebars and view mode stay per window.
- Choosing another Vault closes the other windows, their Notes saved and their sessions forgotten.
