# 27: File operations

**What to build:** ⌘N creates `Untitled.md` (then `Untitled 1.md`…) in the folder of the Note in front with its name ready to type. The Files pane creates, renames and moves Notes and folders, and moves them to the Trash. Renaming does not update Wikilinks in v1.

**Blocked by:** 04 (Write and save a Note)

**Status:** ready-for-agent

- [x] ⌘N creates and opens a new Note with its name selected
- [x] Notes and folders can be created, renamed and moved by dragging
- [x] Delete moves to the macOS Trash, never deletes for good
- [x] Tested at Seam 1

## Comments

Done.
- ⌘N creates `Untitled.md` (`Untitled 1.md`…) in the folder of the Note or Attachment in front, at the root without one, opens it in the Tab in front and selects its name in Files, its folders opened. Return or a click elsewhere names it, Escape leaves it `Untitled`; then the editor takes the keyboard.
- Files' menu, on a row or below them: New Note, New Folder, Rename, Move to Trash. ⌘⌫ moves the row selected to the Trash, only while Files has the keyboard (in the editor it deletes to the start of the line); Delete alone does nothing.
- A row dragged onto a folder moves into it, onto a file into its folder, below the rows to the root.
- Names: a Note's and an Attachment's typed without extension, kept. Refused, with an alert: a name taken in any letter case, empty, starting with a dot, an Excluded folder's (`node_modules`), or with `/ \ : [ ] # ^ |`. A change of letter case only is a rename.
- Moves to the macOS Trash, never deleted for good, without asking (the Finder's Put Back undoes it).
- Every window of the Vault follows Noteki's own renames and moves at once, folders included (the file event of a folder's move names only the folder): Tabs, their history, open Notes, recent Notes, Files' open folders. The Note open is saved first. The Trash closes the Tabs that showed it, except a Note whose text is only on screen, which keeps its Tab and gets the deleted banner. ADR 0010 records the direct link between windows.
- Wikilinks are not updated (v1).

Files' outline is now built from folders the model keeps open (DisclosureGroup), so a row to name can be shown inside a closed folder.

Tests: Seam 1 (Untitled numbering in any case, folders, rename keeping extensions, case-only rename, names refused, moves, a folder into itself, an open Note following a move, paths after a move, the Trash).

Fixed after review:
- a case-only rename could replace another file on a case-sensitive volume;
- renames failed on volumes without exclusive rename (SMB, exFAT);
- a folder renamed `node_modules` vanished;
- the Trash could drop another window's last keys, and left the Note just left and the recent Notes pointing at it;
- the windows followed a change at different moments;
- Delete alone moved files to the Trash.

Fixed after trying it in the app:
- in a closed folder the name field never showed (⌘N from a Note opened with the Quick switcher): Files now opens the folders of the row to name;
- ⌘⌫ did nothing (a ⌘ key is a menu shortcut, it never reached the list): now a shortcut enabled only while Files has the keyboard;
- Rename on a row not selected lost the typing: the row now gets selected and opened first.

Verified in the app, on the demo Vault: ⌘N at the root, named "Plan A" with Return, "hello" typed in the editor and saved there; ⌘N from Meetings/Retro with Meetings closed (the folder opens, the field shows); Escape leaves `Untitled` and the editor takes the keyboard; Rename from the menu, on the row selected and on one not selected; Plan B dragged onto Archive, its Tab and title following, its text kept; ⌘⌫ on a row moves it to the Trash and closes both Tabs that showed it, Delete alone does nothing; ⌘⌫ in the editor deletes to the start of the line, ⌘Z brings it back. Not tried: a drop below the rows to the root, New Folder, a move followed in a second window.
