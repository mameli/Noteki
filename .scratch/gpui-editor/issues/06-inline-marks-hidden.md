# 06: Inline marks hidden, shown inside the element the cursor is in

**What to build:** Bold, italic, bold italic (`***`), strikethrough, highlights, inline code, links and Wikilinks are drawn without their marks: `**`, `*`, `~~`, `==`, the backticks, `[`, `](url)`, `[[`, `]]` and a Wikilink's target before its `|` Alias. A Comment's `%%` too. The marks of one element show while the cursor (or the end of the selection) is inside it or at its edge, only on the cursor's line, as SPEC › Live preview says; the other marks of that line stay hidden. Today every mark of the cursor's line shows: that changes.

**Blocked by:** 04

**Status:** ready-for-agent

- [x] The scanner gives each inline element its range and its marks' ranges (a new function in `core::markdown`, or `compute_markdown_spans` extended), tested on its own: nested emphasis, `***x***`, a link with a title, `[[Note|Alias]]`, an escaped `\|` in a table, code containing `**`
- [x] `style_lines` hides the marks of every element the cursor is not in; inline code keeps its background, a link its colour and underline
- [x] The cursor entering an element shows its marks, leaving it hides them; never a jump on the lines above (SPEC)
- [x] The cursor never stands in a hidden mark: arriving at an element's edge shows its marks, so → through `**a**` steps over every character as written, as in Obsidian
- [x] Every section of `rendering-test.md` up to "Liste" reads as in Obsidian with the cursor elsewhere
- [x] Robustness: for random texts and cursors, hidden ranges are valid, disjoint and never hold the cursor; the display map round-trips
- [x] UI test on `rendering-test.md`: the cursor goes down line by line through the whole Note and no line above it moves

## Comments

Done (2026-11-16).

- No new scanner: `compute_markdown_spans` now takes the cursor's byte offset (`Option<usize>`) and its per-line body became `line_spans`. A mark is a `SyntaxMarker` span unless the cursor is inside its element or at its edge (`inline_tokens` already gave each element's range); a Heading's `#`s still show with the cursor anywhere on its line. Comments' `%%` are marks now too.
- `style_lines` no longer takes the cursor: a line's hidden ranges are its `SyntaxMarker` spans, merged. The Heading-only code of ticket 04 is gone.
- Speed: the editor styles every line once per change of the text with every mark hidden (`base_styles`), then restyles only the cursor's line when the cursor moves (`style_line`). Typing 9.1 ms, an arrow key 5.4 ms, an idle frame 1.75 ms in the harness (which draws two frames per arrow key, four per keystroke).
- Scanner fix: `***x***` was read as `**` + `*x` with a stray `*`; it is now italic around bold, as CommonMark reads it (the Reader benefits too).
- Escaped `\|` in `[[N\|A]]`: shows the Alias, as SPEC says.
- Tests: unit (per-element marks, Wikilink Alias and Comment, `***`, code holding `**`, a link with a title, `\|`), robustness (the cursor never inside a hidden mark), UI (`**` takes no room off the element and shows in it; walking down `tests/fixtures/rendering-test.md` line by line never moves a line above the cursor). The fixture is a copy of the user's test note. Two mutants replace ticket 04's (all marks of the cursor's line shown; the cursor's line never restyled), both killed.
