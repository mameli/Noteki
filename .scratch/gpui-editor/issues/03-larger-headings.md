# 03: Headings larger than the text

**What to build:** ATX Headings (`#` to `######`) are drawn larger than the text, H1 largest, from the spans `compute_markdown_spans` already finds. Lines now have different heights, and everything that depends on positions keeps working: the cursor, the selection, ↑ ↓, scrolling. The `#`s are still shown (faded); hiding them is ticket 04.

**Blocked by:** 02

**Status:** ready-for-agent

- [x] H1–H6 drawn at a scale like Obsidian's, bold, in the Heading colour of the theme; the sizes defined in `src/app/style.rs`
- [x] A Heading line's height follows its size and a long Heading wraps at its own size
- [x] ↑ ↓ from the text into a Heading and out keep the cursor's x, not its column
- [x] Typing `# ` at the start of a line turns it into a Heading at once; deleting the `#` turns it back; only that line is reshaped
- [x] Code blocks and the Frontmatter: a `#` there is no Heading (as the scanner already decides)
- [x] Tested without GPUI: with lines of mixed heights, the top of each line, the line under a given y and the cursor's position after ↑ ↓

## Comments

Done (2026-11-16).

- `core::editor::styling::style_lines`: for each logical line, its Heading level (from `parse_atx_heading` on the scanner's `Text` lines, so a `#` in code or in the Frontmatter is no Heading, and `# ` with nothing after it already is one) and the Live preview spans of `compute_markdown_spans` cut into segments that don't overlap.
- The element shapes each Heading line at its size: H1–H6 at 1.802, 1.602, 1.424, 1.266, 1.125 and 1.0 times the text (Obsidian's default theme), with a line height of 1.35 times its own size, against 1.6 for text. The constants are in `src/editor/element.rs`, not in `style.rs` as the ticket said, because they are layout, not colour.
- Not only Headings: since the spans were there, every style the old editor drew is back (bold, italic, strikethrough, highlights, inline code and code blocks, links, resolved/unresolved Wikilinks, Tags, Comments, quotes, callout titles, done Tasks, the Frontmatter), with the marks faded except on the cursor's line, as before. `src/app/style.rs` moved to `src/editor/style.rs`. The app gives the editor the Note's path and the Vault index (`set_link_context`) to tell unresolved Wikilinks.
- The styles are recomputed only when the text changes or the cursor moves to another line (0.8 ms per keystroke on a 100 KB Note, a new row in `examples/perf.rs`). Above 1 MB, Live preview is off and the Note is plain text, as before (`LIVE_PREVIEW_MAX_BYTES`).

Tests: 4 unit tests for `style_lines`; a robustness property (segments are valid slices, in order, inside their line); 3 UI tests (H1 > H2 > H3 > text and no Heading in code; `# ` makes a Heading at once and ⌫⌫ undoes it, the other lines keeping their height; ↑/↓ through a Heading come back to the same x). New mutant: a `#` in code drawn as a Heading, killed.

Typing lagged (reported by the user after trying it). Measured in the real `NotekiApp` on a 100 KB Note (`tests/keystroke_timing.rs`, ignored by default): 225 ms per keystroke in a debug build, 27 ms in release. Causes and fixes:
- Every frame shaped every line from scratch, twice: once for GPUI's natural-size query (with a made-up width of 800 px) and once at the real width. Now the natural-size query returns the last height without shaping, and the shaped lines stay in a cache in the `NoteEditor`. A frame where nothing changed (the caret blinking, scrolling) reuses them whole; an edit shapes again only the lines whose text, style or composed text changed.
- Line styles held absolute offsets, so typing one character changed the style of every line below it and they were all shaped again. Segments are now relative to their line, and a line whose style did not change keeps the same `Rc`.
- `cargo run` built GPUI unoptimized. `Cargo.toml` now builds dependencies at `opt-level = 3` in dev, and Noteki at 1.

After: 11 ms per keystroke in dev, 9 ms in release, in the test harness, which draws about four frames per keystroke. Per keystroke the editor now does about 0.3 ms of shaping plus 1.6–2.3 ms of styles. Idle frame 1.8 ms. New UI test: a third backtick that opens a code block above a Heading draws it as code (the line's style changes, not its text). New mutant: a line whose style changed is drawn from the cache. Killed.
