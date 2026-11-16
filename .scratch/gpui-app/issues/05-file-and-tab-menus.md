# 05: Context menus on files, folders and Tabs; middle click closes a Tab

**What to build:** A right-click on a file in the tree: Open, Open in new Tab, Rename…, Make a copy, Copy path, Copy full path, Reveal in Finder, Move to Trash. On a folder: New note here, New folder here, Rename…, Copy path, Reveal in Finder, Move to Trash. On a Tab: Close, Close others, Close Tabs to the right, Copy path, Reveal in Finder. A middle click on a Tab closes it. The file toolbar's Rename, Make a copy and Move to Trash buttons stay (they act on the Note in front).

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Menus with GPUI Kit's `Tree::context_menu` and `ContextMenuExt`, items calling `NotekiApp`'s existing actions
- [ ] Copy path: the path in the Vault; Copy full path: the absolute path; Reveal: `cx.reveal_path`
- [ ] `core::tabs`: close others, close to the right (unsaved Notes saved first, as closing one Tab does)
- [ ] Tests: `core::tabs` closing rules; UI (a middle click closes the Tab under it; Copy path writes the clipboard; Make a copy from the menu creates the copy)
