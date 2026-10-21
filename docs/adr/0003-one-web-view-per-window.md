# One web view per window, with Noteki's own Tabs instead of macOS window tabs

macOS gives Tabs for free (NSWindow tabbing), but each native tab is a window, so each would hold its own WKWebView and its own WebKit process of roughly 40–60 MB. Five open Notes would break the 150 MB budget at rest. Noteki draws its own tab bar instead: a window has one WKWebView, and switching **Tab** swaps the CodeMirror editor state, which is instant and keeps each Tab's cursor, scroll and undo. More windows (⌘⇧N) are still possible, each with its own web view.

## Consequences

- The tab bar, its keyboard shortcuts and dragging Tabs between windows are code Noteki writes and maintains.
- Only one Note per window is in the web view at a time, so memory grows with windows, not with Tabs.
- A Tab in the background keeps its text in memory as a CodeMirror state, not the whole rendered view.
