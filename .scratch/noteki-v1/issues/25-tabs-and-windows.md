# 25: Tabs and windows

**What to build:** Noteki's own tab bar: one web view per window, switching Tab swaps the editor state and keeps cursor, scroll and undo per Tab. ⌘-click opens in a new Tab, ⌘W closes, ⌃Tab / ⌃⇧Tab switch, ⌘⌥← / ⌘⌥→ go back and forward, ⌘⇧N opens a window. Tabs are restored at launch.

**Blocked by:** 04 (Write and save a Note)

**Status:** ready-for-agent

- [ ] Several Notes open as Tabs in one window, one web view
- [x] Each Tab keeps its cursor, scroll and undo
- [ ] ⌘-click opens a new Tab; ⌘W, ⌃Tab, ⌃⇧Tab work
- [ ] Back and forward follow the Notes visited in a Tab
- [ ] ⌘⇧N opens a window with its own web view
- [ ] Tabs of each window come back at launch
- [ ] Memory does not grow with the number of Tabs (measured)

## Comments

Built; the in-app check and the memory measure are still to do (below).
- **Tabs**, a pure model in NotekiCore (`Tabs`): what each Tab shows and showed, a new Tab after the one in front, closing (the next comes in front, or the one before), next and previous round, Followed renames of files and folders, kept as JSON in the preferences, and left out when their file is gone at launch.
- **The tab bar** is Noteki's own, above the Note. A click brings a Tab in front, its × closes it.
- **A ⌘-click opens a new Tab** from Files, Wikilinks, Backlinks, search results and the Tags pane; ⌘Enter does it in the Quick switcher. A file opened from the Finder or Takku goes to the Tab that shows it, or to a new Tab.
- **Each Tab keeps its Note open.**
  - The editor keeps one CodeMirror state per Tab, with its cursor, undo and scroll (`restore`, `drop`).
  - The app keeps the Note's text, saved when the Tab is left.
  - A Note behind follows a Followed rename and takes the file's changes (merged and saved if it held keys not saved). The editor applies them when the Tab comes back, outside the undo history.
  - Source mode is per Tab.
- **Commands:** ⌘W closes the Tab, or the window without one; the Go menu has Back, Forward, Show Previous Tab and Show Next Tab.
- **⌘⇧N opens another window** with its own model and web view; the windows keep in step through the files ([ADR 0010](../../../docs/adr/0010-each-window-its-own-model.md)). Choosing another Vault closes the other windows and brings back those kept for the Vault chosen.
- **`scripts/measure TABS=20`** opens 20 Notes in Tabs as a last launch would leave them, brings each in front once, then measures.

Tests: Seam 1 (Tabs: opening, history, closing, cycling, renames, kept and brought back, Tab names; the bridge's JSON) and Seam 2 (the difference applied to a kept state: cursor and undo kept, the file's change outside the history).

Not done, in the SPEC: an Attachment's Tab, and back and forward, do not follow a rename.

Fixed after review:
- a Tab coming back after an Attachment's Tab lost its state;
- reloads from disk were dropped after any Tab switch, the editor counting reports across Notes;
- Tabs brought back at launch did not follow renames;
- a change on disk during a Tab switch could miss the Note going behind;
- closing a ⌘⇧N window with edits not saved lost them;
- a deleted Note behind was closed without asking;
- a Merge behind was not saved;
- ⌘-click on the selected row, or on a Backlink of the Note in front, opened no Tab;
- the Tabs kept for other windows were overwritten at launch and when switching Vault.

Verified on the editor page in the browser: Note A left at line 56, scrolled 992 px, with "XYZ" typed; Note B loaded; A brought back at the same line and scroll; ⌘Z removed "XYZ" and reported it for A.

Not yet verified in the app: Noteki launched in the background opens no window until it is brought forward (as before this ticket), and that would take the keyboard from the user. Every item above, ⌘W against the window's own Close, and `scripts/measure TABS=20` (memory with Tabs, still unticked) wait for the user's go-ahead.
