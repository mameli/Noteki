# 05: Native editing behaviour in the editor

**What to build:** Writing in the web view feels like any Mac text field: find, select-all, spellcheck, autocorrect, input methods, word movement and key equivalents behave as expected, adapting MarkEdit's solutions (MIT, recorded in THIRD_PARTY.md).

**Blocked by:** 04 (Write and save a Note)

**Status:** ready-for-agent

- [x] ⌘F opens a find bar that searches the whole Note, not only the visible part
- [x] Select-all selects the whole Note, also when it is longer than the window
- [x] System spellcheck and autocorrect underline and correct as in other apps
- [x] Accented characters typed with a dead key or held key arrive correctly
- [x] ⌥← / ⌥→ move by word; menu key equivalents do not beep
- [x] Borrowed code is listed in THIRD_PARTY.md

## Comments

Done. Verified in the app on a demo Vault with keystrokes sent through accessibility:
- On a 2,000-line Note, ⌘A then ⌘C copies all 2,000 lines.
- ⌘F, and Edit → Find → Find… with the sidebar focused, find a word at line 1900. ⌘A and ⌘C in the find field act on the field.
- ⌥← twice moves two words back. A dead key (⌥e then e) types é. ⌘Z undoes through the Edit menu.
- Misspelled words are underlined. Edit → Spelling and Grammar toggles reach the editor from the sidebar and show their state.
- Find, Undo and Select All are off while an Attachment is in front.

Not verified by automation: that no key equivalent beeps, the press-and-hold accent popup, and autocorrect. Autocorrect depends on the system setting and real typing. Undo and Redo stay enabled while a Note is in front even when there is nothing to undo, because the editor's history is not reported to the menus. The search panel also has Replace, and CodeMirror's ⌘D, ⌥⌘G and ⇧⌘L.
