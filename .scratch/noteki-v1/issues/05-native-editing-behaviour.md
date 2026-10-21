# 05: Native editing behaviour in the editor

**What to build:** Writing in the web view feels like any Mac text field: find, select-all, spellcheck, autocorrect, input methods, word movement and key equivalents behave as expected, adapting MarkEdit's solutions (MIT, recorded in THIRD_PARTY.md).

**Blocked by:** 04 (Write and save a Note)

**Status:** ready-for-agent

- [ ] ⌘F opens a find bar that searches the whole Note, not only the visible part
- [ ] Select-all selects the whole Note, also when it is longer than the window
- [ ] System spellcheck and autocorrect underline and correct as in other apps
- [ ] Accented characters typed with a dead key or held key arrive correctly
- [ ] ⌥← / ⌥→ move by word; menu key equivalents do not beep
- [ ] Borrowed code is listed in THIRD_PARTY.md
