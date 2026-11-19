# 01: The sidebars and bars as cached views

**What to build:** at every frame (a key, the caret) GPUI renders `NotekiApp` from its root and lays out the whole window again with taffy: the left sidebar (its pane switcher, Files/Tags/Search), the right sidebar (Outline/Backlinks header and list), the title bar, the tab bar and the status bar, all with GPUI Kit's buttons. Only the editor changes while typing. Make each of those parts its own view (an `Entity` that renders it) drawn with GPUI's view caching (`AnyView::cached` with its size as a style), so a frame where only the editor is notified reuses their last layout and paint. The parts notify themselves when what they show changes (the Outline after a pause in typing, the word count, the tabs, the Vault's tree).

**Why:** the last profile of `keystroke_timing` (after 0.4.1's changes) still puts about a quarter of a key in taffy's layout of the rest of the window and in `NotekiApp::render`. Outline and Backlinks already draw only their rows in view (uniform lists); the tree of Files does too.

**Blocked by:** none

**Status:** done

- [x] Left sidebar, right sidebar, title bar, tab bar and status bar are views; `NotekiApp` keeps the state and they read it, or each keeps what it shows and is updated by `NotekiApp`
- [x] Typing notifies only the editor; the rest is notified at the pause (`after_text_change`'s `typing_redraw`) or when its own state changes
- [x] Every UI test passes as it is (element ids and the `test_support` markers stay)
- [x] `keystroke_timing` before and after, in the commit message (both the Note as it is and `NOTEKI_KT_PARAGRAPHS=1`), and a `sample` profile showing taffy and `NotekiApp::render` gone from a key
- [x] Nothing drawn differently: the screenshots of README's demo vault taken again and compared

## Comments

### 19 November 2026: done

- `src/app/parts.rs`: the title bar, both sidebars, the tab bar and the status bar are each a `Part`, a view that renders the part from `NotekiApp` and observes it, drawn with `Entity::cached` at its size (`TITLE_BAR_HEIGHT`, `TAB_BAR_HEIGHT` 33, `STATUS_BAR_HEIGHT` 24, a sidebar its panel). `NotekiApp` keeps the state, and its `cx.notify()` (a pause in typing, a click, a Vault event) draws them again. A hover or an input inside one notifies the part itself. The editor is the only view drawn at a key.
- `keystroke_timing` drew every key with GPUI Kit's `input`, three frames each with `window.refresh()`, which ignores every cached view: the change did not show in it at first. It now dispatches the key and draws one frame without `refresh`, as the app does. Measured that way, before and after: a key in the 112 KB Note 4.13 → 3.3 ms, in paragraphs 2.09 → 1.24 ms, an arrow 1.30 → 0.49 ms (the whole window drawn again took 0.7–0.8 ms of each).
- Every UI test passes unchanged (114). The demo vault drawn before and after is identical to the pixel, apart from the window's traffic lights. The status bar had relied on its parent to stretch it, and now sets its own width.

