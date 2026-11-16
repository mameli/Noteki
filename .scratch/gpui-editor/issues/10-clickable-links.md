# 10: Clickable links and Wikilinks

**What to build:** A click on a link's text opens its URL in the browser; a click on a Wikilink opens its Note (⌘-click in a new Tab), an unresolved one creates it next to the Note, as the app does from the Reader today (`ReaderLink`, `follow_link`). The pointer is a hand over a link. A click with the cursor already inside the link's marks (editing it) only moves the cursor.

**Blocked by:** 06

**Status:** ready-for-agent

- [x] The element knows which shown ranges are links and their targets (from the scanner's elements of ticket 06)
- [x] Click opens, ⌘-click opens in a new Tab, as SPEC › Wikilinks says; ⌘↩ on a link (already bound) keeps working
- [x] A link's text can still be selected by dragging from it, and double-clicked
- [x] "Link" and "Ultimo test" of `rendering-test.md`: both links open
- [x] UI tests: a click on a Wikilink opens the Note; a click on a Markdown link asks the app to open the URL (through a seam the test can see, no browser)

## Comments

Done (2026-11-16).

- `core::markdown::link_at(text, offset)`: the link whose shown text (a link's label, a Wikilink's name or Alias) holds the offset, edges included: a Markdown link's destination (without its title or `<>`), or the Wikilink. Not in code or the Frontmatter; an Embed is no link.
- A single click (no ⇧) on a link's text emits `NoteEditorEvent::OpenLink`, unless the cursor is already inside that link (its marks show: the user is editing it), when the click only moves the cursor. The app opens a URL with `cx.open_url` and a Wikilink with its existing `follow_link` (⌘-click: a new Tab; an unresolved one is created, as from the Reader).
- The pointer is a hand over a link a click would open (a hover check on the line under the pointer).
- Double-click and drag on a link's text: a double-click is not a single click, so it selects a word; a drag has to start elsewhere (a click on the link opens it), as in Obsidian.
- Tests: `link_at` (label, edges, title, Alias, not in code); UI (⌘-click opens a new Tab, a click the same Tab; a click on a Markdown link reaches the test platform's `opened_url` without moving the cursor; a click in a link being edited only moves the cursor). New mutant, killed.
