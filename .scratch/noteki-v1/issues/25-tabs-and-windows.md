# 25: Tabs and windows

**What to build:** Noteki's own tab bar: one web view per window, switching Tab swaps the editor state and keeps cursor, scroll and undo per Tab. ⌘-click opens in a new Tab, ⌘W closes, ⌃Tab / ⌃⇧Tab switch, ⌘⌥← / ⌘⌥→ go back and forward, ⌘⇧N opens a window. Tabs are restored at launch.

**Blocked by:** 04 (Write and save a Note)

**Status:** ready-for-agent

- [x] Several Notes open as Tabs in one window, one web view
- [x] Each Tab keeps its cursor, scroll and undo
- [x] ⌘-click opens a new Tab; ⌘W, ⌃Tab, ⌃⇧Tab work
- [x] Back and forward follow the Notes visited in a Tab
- [x] ⌘⇧N opens a window with its own web view
- [x] Tabs of each window come back at launch
- [x] Memory does not grow with the number of Tabs (measured)

## Comments

Built, then checked in the app and measured (end of this section).
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

Checked in the app, 26 October 2026, on a test Vault:
- three Tabs opened with ⌘-click from Files; ⌃Tab, ⌃⇧Tab and the tab bar bring each in front; ⌘-click on a Wikilink opens a Tab, ⌘W closes it, then the window once it has no Tab; the window's own Close has no shortcut, so ⌘W is only Close Tab;
- Long left scrolled to line 58 with the cursor on line 75, brought back at the same line; text typed after each switch lands in the Note in front; ⌘Z undoes in each Tab only what was typed there;
- back and forward through a Wikilink followed in a Tab, also after a relaunch;
- ⌘⇧N opens a second window with its own web view (one more WebContent process); the same Note open in both stays in step through the file;
- after quitting with two windows, both come back with their Tabs and the Note in front of each; a file changed while Noteki was closed is read again.

Fixed during that check:
- ⌃Tab did nothing while the editor had the keyboard, and moved the keyboard to Files (WebKit takes ⌃Tab to move between views): ⌃Tab and ⌃⇧Tab now go to the Go menu wherever the keyboard is;
- a Tab came back scrolled half a line off: its scroll is kept as the line at the top, not in pixels, which CodeMirror only estimates for lines not drawn;
- a click in the empty space after a line ending with a Wikilink followed the link (since ticket 11): only a click on the line's text follows it.

A closed window's model and web view stay in memory until a menu is next opened or a shortcut used: SwiftUI keeps the menus' actions, built while that window was in front, until it builds them again.

Memory with 20 Tabs, in CHANGELOG.md: Noteki's own process grows by 4 MB, WebKit's WebContent by 11 to 38 MB, inside its spread from run to run; one web view for all the Tabs. The start is 0.04–0.14 s slower with 20 Tabs to bring back. Today's totals are over the budget with or without Tabs: WebKit's GPU process takes about 100 MB more whenever the editor has the keyboard in the active window, which the earlier measures never had (CHANGELOG.md).
