# 02: Soft wrap and scrolling

**What to build:** Long lines wrap to the width of the window, and a Note longer than the window scrolls. ↑ and ↓ move by wrapped row, not by logical line, and keep the column the cursor started from. The cursor always stays in view.

**Blocked by:** 01

**Status:** ready-for-agent

- [x] Each line is shaped with the window's width as wrap width (`shape_text`); resizing the window rewraps it
- [ ] The maximum line width of Settings is respected, the text centred as in the app
- [x] ↑ ↓ move by wrapped row and keep the x the cursor had when the vertical movement began, until a horizontal move or an edit
- [x] Trackpad and mouse wheel scroll, with momentum; typing or moving the cursor out of view scrolls it back into view
- [x] Only the lines in view (and a margin) are shaped and drawn: a 1 MB Note scrolls and types within a frame (measured in `examples/perf.rs`)
- [x] Tested without GPUI: given the heights of the lines and a scroll position, which lines are in view and where the cursor is

## Comments

Done (2026-11-16), except the line width.

- Wrapping, ↑/↓ by wrapped row and native scrolling were done in ticket 01.
- **The cursor stays in view.** After a key or an edit (not a click or a drag), the next frame scrolls as little as needed to show the cursor with one line of margin. The rule is `core::editor::lines::scroll_to_reveal`, tested without GPUI. Reloading a Note changed by another app keeps the scroll; switching to another Note starts at the top and then shows its cursor.
- **⌘← ⌘→ (and Home/End, ⌘⌫, ⌘⌦) go to the ends of the wrapped row**, as in macOS. A row that wraps ends before the space it wraps at. ⌃A, ⌃E and ⌃K stay on the logical line (the paragraph), as Cocoa does.
- **Only the visible lines are drawn, but every line is still shaped at every frame.** Measured with macOS's real text system in `examples/perf.rs`, which now has editor budgets:

  | Note | First frame (opening it) | A later frame |
  |---|---:|---:|
  | 100 KB | 48 ms | 0.2 ms |
  | 1 MB | 404 ms | 1.8 ms |

  GPUI keeps shaped lines from one frame to the next, so typing and scrolling only look lines up, and a keystroke reshapes one line. Shaping only the visible lines would help just the first frame and resizing of very large Notes. The cost is estimated heights for the lines not yet seen, and a scroll that jumps as they get measured. Not worth it now: revisit if Notes above about 300 KB feel slow to open or resize.
- **Line width**: there is no such setting in the prototype. The text is 820 px wide at most, centred, as before. Left for when Settings get one.

Tests: `scroll_to_reveal` in `core::editor::lines`; UI tests for the view following ⌘↓/⌘↑ (and not moving for ↑ inside the view), and for ⌘→/⌘←/⌃E/⌃A on a wrapped line. Three new mutants in `scripts/mutants.sh` (composed text not kept, view not following the cursor, ⌘→ past the wrap space), all killed.
