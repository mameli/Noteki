# 01: The sidebars and bars as cached views

**What to build:** at every frame (a key, the caret) GPUI renders `NotekiApp` from its root and lays out the whole window again with taffy: the left sidebar (its pane switcher, Files/Tags/Search), the right sidebar (Outline/Backlinks header and list), the title bar, the tab bar and the status bar, all with GPUI Kit's buttons. Only the editor changes while typing. Make each of those parts its own view (an `Entity` that renders it) drawn with GPUI's view caching (`AnyView::cached` with its size as a style), so a frame where only the editor is notified reuses their last layout and paint. The parts notify themselves when what they show changes (the Outline after a pause in typing, the word count, the tabs, the Vault's tree).

**Why:** the last profile of `keystroke_timing` (after 0.4.1's changes) still puts about a quarter of a key in taffy's layout of the rest of the window and in `NotekiApp::render`. Outline and Backlinks already draw only their rows in view (uniform lists); the tree of Files does too.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Left sidebar, right sidebar, title bar, tab bar and status bar are views; `NotekiApp` keeps the state and they read it, or each keeps what it shows and is updated by `NotekiApp`
- [ ] Typing notifies only the editor; the rest is notified at the pause (`after_text_change`'s `typing_redraw`) or when its own state changes
- [ ] Every UI test passes as it is (element ids and the `test_support` markers stay)
- [ ] `keystroke_timing` before and after, in the commit message (both the Note as it is and `NOTEKI_KT_PARAGRAPHS=1`), and a `sample` profile showing taffy and `NotekiApp::render` gone from a key
- [ ] Nothing drawn differently: the screenshots of README's demo vault taken again and compared

## Comments
