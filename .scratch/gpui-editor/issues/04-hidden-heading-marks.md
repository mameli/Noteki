# 04: Heading marks hidden outside the cursor's line

**What to build:** The `#`s of a Heading and the space after them are hidden, not faded, unless the cursor or the end of the selection is on the Heading's line (SPEC › Live preview). This brings in the **display map** between the Note as written and what is shown: the first time the new element shows text that is not in the buffer as it is. Every later mark (emphasis, links, Wikilinks) will use the same map.

**Blocked by:** 03

**Status:** ready-for-agent

- [x] On a Heading's line without the cursor, the text starts at the left edge, no `#` drawn
- [x] Moving the cursor onto the line shows the `#`s; moving off hides them; the lines above the cursor never move (SPEC: the text above the cursor does not jump)
- [x] ← from the start of the Heading's text on another line, and ↑ ↓ onto the Heading, put the cursor where the user expects, never inside a hidden range
- [x] The display map converts offsets both ways; a Heading line is reshaped only when its hidden ranges change
- [x] In `tests/robustness.rs`: for random texts and cursors, mapping a shown offset to the Note and back gives the same offset, the Note's offset is never inside a hidden range, and the cursor's line hides nothing

## Comments

Done (2026-11-16).

- **The display map** (`core::editor::display`): a line's hidden ranges (relative to its start, sorted), its shown text (`shown_text`), and offsets both ways. `to_display` puts an offset inside a hidden range where the range was cut out. `to_buffer` puts a shown offset at a cut after the hidden range, so a click or ↑/↓ before a Heading's first letter lands before that letter, never inside or before its `# `. Every later mark (emphasis, links, Wikilinks) can use the same map: it only needs more hidden ranges.
- `style_lines` takes the cursor's line and gives each line its `hidden` ranges: for an ATX Heading off the cursor's line, the indent, the `#`s and the space after them. The element shapes the shown text, gives the hidden ranges no run, and maps every position (caret, selection, clicks, ↑/↓, IME bounds) through the map. The hidden ranges are part of the line's style, so the shape cache shapes a line again exactly when they change.
- **The cursor is never inside a hidden range by construction**: only the lines without the cursor hide anything. A selection's anchor can be inside one, on another line; it is then drawn where the range was cut out.
- **No jump above the cursor**: hiding or showing `# ` changes the width of one line, never the height of the lines above (a Heading so long that `# ` makes it wrap one more row changes its own height, which SPEC allows for the line the cursor enters or leaves).
- What the ticket said differently: "← from the start of the Heading's text on another line" does not apply, since ← never leaves the cursor's line hidden; the round trip holds from shown offsets (a Note's offset at the start of a hidden range shares its shown offset with the range's end).

Tests: 3 unit tests for the map, 1 for the hidden ranges; a robustness property (hidden ranges valid and sorted, none on the cursor's line, shown offsets round-trip and never land inside a hidden range); 2 UI tests (`## ` hidden off the cursor's line, ↑ lands after it and shows it, ↓ hides it again; moving the cursor up through Headings never moves the lines above it). Two new mutants (a click lands before the hidden `# `; marks hidden on the cursor's line), both killed. Keystroke timing unchanged (9 ms per keystroke in the harness, 1.8 ms per idle frame).
