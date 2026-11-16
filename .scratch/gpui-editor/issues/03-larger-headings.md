# 03: Headings larger than the text

**What to build:** ATX Headings (`#` to `######`) are drawn larger than the text, H1 largest, from the spans `compute_markdown_spans` already finds. Lines now have different heights, and everything that depends on positions keeps working: the cursor, the selection, ↑ ↓, scrolling. The `#`s are still shown (faded); hiding them is ticket 04.

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] H1–H6 drawn at a scale like Obsidian's, bold, in the Heading colour of the theme; the sizes defined in `src/app/style.rs`
- [ ] A Heading line's height follows its size and a long Heading wraps at its own size
- [ ] ↑ ↓ from the text into a Heading and out keep the cursor's x, not its column
- [ ] Typing `# ` at the start of a line turns it into a Heading at once; deleting the `#` turns it back; only that line is reshaped
- [ ] Code blocks and the Frontmatter: a `#` there is no Heading (as the scanner already decides)
- [ ] Tested without GPUI: with lines of mixed heights, the top of each line, the line under a given y and the cursor's position after ↑ ↓
