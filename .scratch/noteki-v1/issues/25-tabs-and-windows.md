# 25: Tabs and windows

**What to build:** Noteki's own tab bar: one web view per window, switching Tab swaps the editor state and keeps cursor, scroll and undo per Tab. ⌘-click opens in a new Tab, ⌘W closes, ⌃Tab / ⌃⇧Tab switch, ⌘⌥← / ⌘⌥→ go back and forward, ⌘⇧N opens a window. Tabs are restored at launch.

**Blocked by:** 04 (Write and save a Note)

**Status:** ready-for-agent

- [ ] Several Notes open as Tabs in one window, one web view
- [ ] Each Tab keeps its cursor, scroll and undo
- [ ] ⌘-click opens a new Tab; ⌘W, ⌃Tab, ⌃⇧Tab work
- [ ] Back and forward follow the Notes visited in a Tab
- [ ] ⌘⇧N opens a window with its own web view
- [ ] Tabs of each window come back at launch
- [ ] Memory does not grow with the number of Tabs (measured)
