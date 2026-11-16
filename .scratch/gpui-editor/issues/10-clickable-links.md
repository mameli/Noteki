# 10: Clickable links and Wikilinks

**What to build:** A click on a link's text opens its URL in the browser; a click on a Wikilink opens its Note (⌘-click in a new Tab), an unresolved one creates it next to the Note, as the app does from the Reader today (`ReaderLink`, `follow_link`). The pointer is a hand over a link. A click with the cursor already inside the link's marks (editing it) only moves the cursor.

**Blocked by:** 06

**Status:** ready-for-agent

- [ ] The element knows which shown ranges are links and their targets (from the scanner's elements of ticket 06)
- [ ] Click opens, ⌘-click opens in a new Tab, as SPEC › Wikilinks says; ⌘↩ on a link (already bound) keeps working
- [ ] A link's text can still be selected by dragging from it, and double-clicked
- [ ] "Link" and "Ultimo test" of `rendering-test.md`: both links open
- [ ] UI tests: a click on a Wikilink opens the Note; a click on a Markdown link asks the app to open the URL (through a seam the test can see, no browser)
