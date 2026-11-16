# NotekiGPUI: Noteki's own editor element

Status: ready-for-agent

Terms in **bold** are defined in `CONTEXT.md`. The behaviour of **Live preview** is in `docs/SPEC.md` › Live preview; this spec does not override it. It is about the GPUI prototype in `NotekiGPUI/` only. The prototype is free to break and change completely: the Swift app on `main` is the stable Noteki, so nothing here has to keep the prototype usable along the way.

## Problem Statement

The GPUI prototype edits a **Note** with GPUI Kit's `Editor`. `NotekiGPUI/EVALUATION.md` says it can colour and fade Markdown but cannot do what Live preview needs. GPUI's `HighlightStyle` has no font size, so a **Heading** cannot be larger than the text. The editor cannot hide marks, only fade them. And it cannot draw anything that is not text in place: tables, images, formulas.

Patching it is not a way out. The editor is about 38,000 lines (`gpui-base-0.10.0/src/input`), and every row has the same height everywhere in it: `line_height` is used in 209 places, and positions are computed as `line_height * row` (`base/layout.rs`). Giving lines their own height would mean changing that assumption all through someone else's code, and keeping the change up to date with each new version of GPUI Kit.

## Solution

Noteki gets its own editor element, written on GPUI's primitives, built from the start for lines of different heights and for text that is shown differently from how it is written. It starts with what gives the most for the least: plain text editing, then Headings that are larger and have their `#`s hidden.

What it reuses, and why:

- **GPUI**: text shaping and wrapping (`TextSystem::shape_text` returns `WrappedLine`s, each shaped at its own font size), drawing, and native text input through `EntityInputHandler` / `ElementInputHandler` (`gpui-fast/src/input.rs`). Native text input gives accents, dead keys, the macOS press-and-hold accent menu, input methods and dictation without Noteki writing them.
- **`unicode-segmentation`** for moving by character. The text buffer is a plain `String`, not `ropey`: `compute_markdown_spans`, autosave and Merge all need the whole Note as one `&str` at each change, so a rope would be turned back into a `String` on every keystroke. Revisit if typing in a large Note gets slow.
- **`core::markdown`**: `compute_markdown_spans` already finds Headings, emphasis and marks for a text and a cursor, without GPUI, and is tested. The editor draws from it. It does not get a second scanner.
- **GPUI Kit's editor as a reference** (Apache-2.0): pieces such as word movement may be copied and adapted, each one listed in `THIRD_PARTY.md`, as ADR 0007 does for MIT code. It is not a dependency of the new element.

The new element replaces GPUI Kit's `Editor` in `NotekiApp` from ticket 01, and the old editor is removed. Whatever the old one gave and the new one does not have yet (undo, copy and paste, find, the inline styles, the format shortcuts) is missing until its own ticket. That is accepted.

## User Stories

1. As the user, I want to type, delete and move the cursor in a Note as in any Mac text view, so that the new editor never feels foreign.
2. As the user, I want accents, dead keys, the press-and-hold accent menu and dictation to work, so that I can write in Italian.
3. As the user, I want Headings drawn larger than the text, the larger the higher their level, so that I see the structure of a Meeting note at a glance.
4. As the user, I want the `#`s of a Heading hidden unless my cursor is on its line, so that the Note reads as a document.
5. As the user, I want the text above the cursor never to jump when marks appear or hide, so that reading and typing stay calm (SPEC › Live preview).
6. As the user, I want long lines to wrap to the width of the window, so that I never scroll sideways.
7. As the user, I want to click to place the cursor, drag or ⇧-click to select, and scroll with the trackpad, so that the mouse works as I expect.
8. As the user, I want the cursor to keep its column when I move up and down across lines of different heights, so that ↑ and ↓ go where I expect.

## Implementation Decisions

- **Two layers, as the rest of the prototype**: the rules in `src/core/editor/` with no GPUI (buffer, edits, cursor and selection, movement, the map between the written text and what is shown), and the element in `src/editor/` that measures, draws and takes input. The rules are tested as plain Rust. The element only connects them to the window.
- **Lines of any height are the base model, not a special case.** Each logical line (from one `\n` to the next) is shaped as a block at its own font size and wrap width. Its height is stored, and the top of each line is the sum of the heights before it. Clicking, scrolling, the cursor and ↑/↓ all go through these positions, never through `line_height * row`. Shaped lines are cached by line and invalidated by edits, a change in width, or a change in what is hidden on that line.
- **Written text and shown text are separate.** The buffer always holds the Note as written. What is drawn is computed from it and from the cursor: some ranges hidden, some styled. A **display map** converts offsets both ways, so that a click on shown text lands at the right offset in the Note and the cursor is drawn at the right place. A hidden range is never a place the cursor can stop in the middle of.
- **The cursor and selection are byte offsets into the buffer**, always on a character boundary (the robustness tests already check this property for spans). Movement is by grapheme cluster, so that `è` written as `e` + combining accent and emoji are one step.
- **Heading sizes**: a scale like Obsidian's, H1 largest down to H6 close to the text size. The values live next to the other Live preview styles in `src/app/style.rs`.
- **Seam with the app**: `NotekiApp` talks to the editor through a small interface: set and read the text, set and read the cursor, and an event when the text changes (autosave, Merge and reload depend on it). That is what the app uses of GPUI Kit's `EditorState` today (`editor_text`, `set_editor_text`, `set_editor_cursor`, `on_editor_event`). Live preview styles come from `compute_markdown_spans` inside the editor, so `update_decorations` and `TextDecoration` go away.

## Testing Decisions

- A good test states what the user sees or gets, through the narrowest public interface that shows it: an edit and its resulting text and cursor; a text and a cursor and what is hidden; measured heights and a click position and the offset it gives. Not how a function is built.
- `src/core/editor/` is tested as `src/core` is today: unit tests for the cases, and properties in `tests/robustness.rs`. Random edits never panic, the cursor is always on a char boundary and never inside a hidden range, and for every offset, mapping to the shown text and back gives the same offset.
- The element is tested headless in `tests/ui.rs`, through the real `NotekiApp`: typing, ↑/↓ across a Heading, a click on a Heading's line. The existing UI tests (autosave, Merge, Conflict, reload) are kept and moved onto the new editor: they are what shows the swap broke nothing that matters.
- No test brings Noteki to the front or sends real keystrokes while the user may be typing.

## Out of Scope

For these tickets: undo and redo, copy, cut and paste, find, spellcheck, the format shortcuts (⌘B, ⌘I, ⌘1–⌘6), clicking Wikilinks and Tasks in the editor, and every Live preview element other than ATX Headings (emphasis, links, Wikilinks, lists, tables, code blocks, callouts, images). They stop working in the editor when the old one is removed, and come back later, each as a ticket of its own on the same model. Setext Headings are drawn as text. Reader mode does not change.

## Further Notes

- The decision to write Noteki's own editor element rather than patch GPUI Kit's is worth recording as an ADR once ticket 03 shows the model holds.
- The 5 MB rule (SPEC › Live preview: a larger Note opens in Source mode) applies to the new element as to the old one.
- Tickets are in `issues/`, in order. Each one ends with something the user can try in the app (`cargo run`).
