# 06: Back and forward with the mouse and from the editor

**What to build:** The mouse's back and forward buttons (and a trackpad's equivalent, which macOS sends as the same events where configured) go back and forward through the Notes followed, as ⌘⌥← and ⌘⌥→ do. These keys work with the editor focused too (⌘[ and ⌘] outdent and indent there, as SPEC says).

**Blocked by:** none

**Status:** ready-for-agent

- [x] Mouse back/forward buttons anywhere in the window
- [x] ⌘⌥← / ⌘⌥→ go back and forward from the editor (not taken by the editor's word movement)
- [x] Tests: UI (follow a link, back button returns, forward button goes again)

## Comments

Done (2026-11-17). The window's root listens for `MouseButton::Navigate(Back / Forward)` and goes back or forward, wherever the pointer is. ⌘⌥← / ⌘⌥→ already worked from the editor (it binds ⌥← and ⌘←, not ⌘⌥←); now a test proves it. Trackpad swipes reach the app as these buttons only if macOS is set to; GPUI has no swipe gesture of its own. Test: UI with real side-button events, and ⌘⌥← with the editor focused. Mutation check 32/32.
