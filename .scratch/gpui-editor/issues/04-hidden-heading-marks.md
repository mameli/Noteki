# 04: Heading marks hidden outside the cursor's line

**What to build:** The `#`s of a Heading and the space after them are hidden, not faded, unless the cursor or the end of the selection is on the Heading's line (SPEC › Live preview). This brings in the **display map** between the Note as written and what is shown: the first time the new element shows text that is not in the buffer as it is. Every later mark (emphasis, links, Wikilinks) will use the same map.

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] On a Heading's line without the cursor, the text starts at the left edge, no `#` drawn
- [ ] Moving the cursor onto the line shows the `#`s; moving off hides them; the lines above the cursor never move (SPEC: the text above the cursor does not jump)
- [ ] ← from the start of the Heading's text on another line, and ↑ ↓ onto the Heading, put the cursor where the user expects, never inside a hidden range
- [ ] The display map converts offsets both ways; a Heading line is reshaped only when its hidden ranges change
- [ ] In `tests/robustness.rs`: for random texts and cursors, mapping an offset to shown text and back gives the same offset, and the cursor is never inside a hidden range
