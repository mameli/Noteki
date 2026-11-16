# 09: Code blocks

**What to build:** A fenced code block is drawn as a box (the code background, rounded, across all its lines) in the code font, its fences hidden while the cursor is outside the block (the lines stay, so nothing moves), the language written after the fence shown as a small label in the box's corner, its code coloured for the main languages listed in SPEC › Live preview. An indented code block is a box too, without a label. A ```` ```mermaid ```` block is a code block like any other (Mermaid is out of scope).

**Blocked by:** 07

**Status:** ready-for-agent

- [ ] The box is a line decoration (first, middle, last line of the block), so the block keeps its height with the cursor in or out
- [ ] Fences hidden off the block, shown when the cursor enters it (any of its lines, not only the fence's)
- [ ] Syntax colouring with `syntect` (already in the build through GPUI Kit), cached per block and recomputed only when the block changes; a language syntect does not know gets no colouring
- [ ] Code lines never take inline Live preview (no emphasis or links inside code), as the scanner already decides
- [ ] "Blocchi di codice" (Swift) and the Mermaid block of `rendering-test.md` draw as a box with their label
- [ ] Keystroke timing on a Note with many code blocks stays within ticket 03's numbers
