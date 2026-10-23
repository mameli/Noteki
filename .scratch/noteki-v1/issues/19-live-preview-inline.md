# 19: Live preview of inline syntax

**What to build:** The editor shows Notes formatted: Headings, emphasis, inline code, links and Wikilinks render, and their syntax appears only when the cursor is inside that element. ⌘E switches to Source mode. Notes above 5 MB open in Source mode. This sets up the Seam 2 tests.

**Blocked by:** 11 (Vault index and clickable Wikilinks)

**Status:** ready-for-agent

- [x] Bold, italic, inline code, links, Wikilinks and Headings render with their marks hidden
- [x] The marks of an element appear when the cursor enters it, not on the whole line
- [x] Showing or hiding marks never changes the height of lines above the cursor
- [x] ⌘E toggles Source mode
- [x] Tested at Seam 2: given a text and a cursor, what is hidden

## Comments

Done. Live preview hides the marks of bold, italic, strikethrough, inline code (shown with a background), links, autolinks, Wikilinks (their name, or their Alias) and ATX Headings. The marks of an element appear when the cursor is in it or at its edge, only on the cursor's line; a selection shows them on the line where it ends. Code blocks, Embeds, images, reference links, links across lines, HTML and the Frontmatter are left as written for now.

Source mode: ⌘E in the View menu, for the window until Tabs exist (ticket 25), not remembered. A Note over 5,000,000 bytes opens in Source mode, or switches to it when it grows past that, with the menu item checked and disabled.

Seam 2 is set up: `npm test` in `Editor/` bundles `Editor/test/*.test.ts` with esbuild and runs them with Node's test runner on CodeMirror's state with Noteki's language, no browser. `hiddenRanges(state)` is what they test: given a text and a cursor or selection, what is hidden. Ten tests, including one that moves the cursor through every position of a sample and checks that no line above it changes. Not part of the Xcode build, which needs no Node (ADR 0004). The editor's language setup moved to `language.ts`, shared by the editor and the tests.

Not done, in the SPEC: setext Headings keep their underline; the line the cursor leaves can change height (only the lines above the cursor are promised not to).

Fixed after review: a link with its title on the next line crashed Live preview (a hidden range across a line break), `[[Note|]]` hid everything, a selection or ⌘A showed marks on the lines above it, `[[Note\|alias]]` in a table showed the backslash, the tree was walked twice, and the 5 MB rule was only checked on opening.

Verified in the app by typing: marks hidden, only the bold's marks shown with the cursor in it, the first Heading's `#` shown on its line and hidden when the cursor leaves, ⌘E showing every mark, a 6 MB Note opening in Source mode with the menu item checked and disabled.
