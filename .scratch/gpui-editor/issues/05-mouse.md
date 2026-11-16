# 05: Click, drag and select with the mouse

**What to build:** A click puts the cursor at the offset under the pointer, on lines of any height and with hidden marks. Dragging selects, ⇧-click extends the selection, a double-click selects a word, a triple-click the line. Dragging past the top or bottom edge scrolls.

**Blocked by:** 04

**Status:** ready-for-agent

- [ ] A click on shown text lands at the right offset in the Note (through the display map); a click on a Heading's line puts the cursor before the character that was clicked, even though its `#`s then appear and shift the line to the right
- [ ] A click past the end of a line puts the cursor at its end; below the last line, at the end of the Note
- [ ] Drag, ⇧-click, double-click (word), triple-click (line)
- [ ] Dragging past the edge of the view scrolls it, faster the further the pointer is
- [ ] The pointer is an I-beam over text
- [ ] Headless test: a click at a position on a Heading and on a wrapped line gives the expected cursor
