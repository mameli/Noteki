# 05: Context menus on files, folders and Tabs; middle click closes a Tab

**What to build:** A right-click on a file in the tree: Open, Open in new Tab, Rename…, Make a copy, Copy path, Copy full path, Reveal in Finder, Move to Trash. On a folder: New note here, New folder here, Rename…, Copy path, Reveal in Finder, Move to Trash. On a Tab: Close, Close others, Close Tabs to the right, Copy path, Reveal in Finder. A middle click on a Tab closes it. The file toolbar's Rename, Make a copy and Move to Trash buttons stay (they act on the Note in front).

**Blocked by:** none

**Status:** ready-for-agent

- [x] Menus with GPUI Kit's `Tree::context_menu` and `ContextMenuExt`, items calling `NotekiApp`'s existing actions
- [x] Copy path: the path in the Vault; Copy full path: the absolute path; Reveal: `cx.reveal_path`
- [x] Close others, close to the right: closing Tabs one by one with `close_tab` (each saved as when closed alone); nothing new in `core::tabs` was needed
- [x] Tests: `core::tabs` closing rules; UI (a middle click closes the Tab under it; Copy path writes the clipboard; Make a copy from the menu creates the copy)

## Comments

Done (2026-11-17).

- Tree: GPUI Kit's `Tree::context_menu`, per entry. A file: Open, Open in new tab, Rename…, Make a copy, Copy path, Copy full path, Reveal in Finder, Move to Trash. A folder: New note here, New folder here, Rename…, Copy path, Copy full path, Reveal in Finder, Move to Trash. Each item calls the app's existing action (`open_path`, the name prompt, `duplicate`, `trash`) or a new small one (`copy_path`, `reveal`).
- Tabs: a middle click anywhere on a Tab closes it (`on_mouse_down(MouseButton::Middle)`); a right-click on its label: Close, Close others, Close tabs to the right, Copy path, Reveal in Finder. The menu is on the label (GPUI Kit's `TabBar` takes `Tab`s, so the menu cannot wrap the whole Tab).
- Tests: UI with real events (a middle click closes the Tab under it; a right-click on a file shows its menu and its "Make a copy" makes the copy; a right-click on a Tab shows its menu), plus close others / to the right and both copy paths. New mutant: the middle click not closing. Killed.
