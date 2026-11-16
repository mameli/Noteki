# 09: Code blocks

**What to build:** A fenced code block is drawn as a box (the code background, rounded, across all its lines) in the code font, its fences hidden while the cursor is outside the block (the lines stay, so nothing moves), the language written after the fence shown as a small label in the box's corner, its code coloured for the main languages listed in SPEC › Live preview. An indented code block is a box too, without a label. A ```` ```mermaid ```` block is a code block like any other (Mermaid is out of scope).

**Blocked by:** 07

**Status:** ready-for-agent

- [x] The box is a line decoration (first, middle, last line of the block), so the block keeps its height with the cursor in or out
- [x] Fences hidden off the block, shown when the cursor enters it (any of its lines, not only the fence's)
- [x] Syntax colouring with `syntect` (already in the build through GPUI Kit), cached per block and recomputed only when the block changes; a language syntect does not know gets no colouring
- [x] Code lines never take inline Live preview (no emphasis or links inside code), as the scanner already decides
- [x] "Blocchi di codice" (Swift) and the Mermaid block of `rendering-test.md` draw as a box with their label
- [x] Keystroke timing on a Note with many code blocks stays within ticket 03's numbers

## Comments

Done (2026-11-16), for fenced blocks.

- `core::editor::code::highlight`: syntect with `two_face`'s syntaxes (the set GPUI Kit already builds in, so the binary does not grow), loaded once on the first block. TextMate scopes map to six new span kinds (comment, string, number, keyword, type, function) that the theme colours. An unknown language, or none, gets no colours.
- `style_lines` finds the fenced blocks, colours each one's code as one piece, and gives each of its lines a `Decoration::Code` part (opening fence with its language, body, closing fence). The element paints the box across the text's width (rounded at the fences), the language as a small label at the top right, and draws code in Menlo (inline code too), 12 px inside the box.
- Fences are `SyntaxMarker`s off the block. With the cursor on any line of a block, the editor restyles the block's two fence lines to show them (found from the base styles' parts, so still a line or two per cursor move).
- Speed: the editor keeps each block's colours by a hash of its language and code, so typing in one block highlights only that block. On a 112 KB Note with 75 Swift blocks: typing 9.2 ms, an arrow 5.4 ms, an idle frame 1.7 ms in the harness, as before.
- Not done: indented code blocks (four spaces) are not boxes yet. The test note has none, and the scanner does not tell them from a list item's continuation.
- Tests: highlighting (Swift from the test note; an unknown language and any case); the box parts, hidden fences and colours per line; UI (heights the same with the cursor in or out of the block, both fences shown inside and hidden again outside). New mutant: fences never shown in the block. Killed.
