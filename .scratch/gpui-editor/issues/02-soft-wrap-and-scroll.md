# 02: Soft wrap and scrolling

**What to build:** Long lines wrap to the width of the window, and a Note longer than the window scrolls. ↑ and ↓ move by wrapped row, not by logical line, and keep the column the cursor started from. The cursor always stays in view.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] Each line is shaped with the window's width as wrap width (`shape_text`); resizing the window rewraps it
- [ ] The maximum line width of Settings is respected, the text centred as in the app
- [ ] ↑ ↓ move by wrapped row and keep the x the cursor had when the vertical movement began, until a horizontal move or an edit
- [ ] Trackpad and mouse wheel scroll, with momentum; typing or moving the cursor out of view scrolls it back into view
- [ ] Only the lines in view (and a margin) are shaped and drawn: a 1 MB Note scrolls and types within a frame (measured in `examples/perf.rs`)
- [ ] Tested without GPUI: given the heights of the lines and a scroll position, which lines are in view and where the cursor is
