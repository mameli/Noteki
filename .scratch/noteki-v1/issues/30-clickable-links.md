# 30: Clickable links

**What to build:** Links in a Note open on click: `[GitHub](https://github.com)` in the default browser, `[[Noteki test]]` as the linked Note.

**Status:** ready-for-agent

- [x] A Markdown link with a URL opens in the browser
- [x] A wikilink opens its Note on ⌘-click; a plain click places the cursor

## Comments

From user feedback 2026-10-31: "I link non si possono premere".

Done. A Markdown link `[text](url)` and an Autolink `<url>` open in the browser on a click, as a Wikilink already followed; only http, https and mailto, checked again by the app (`openURL`). With the cursor already in the link a click places the cursor, and ⌘-click follows it (a Wikilink in a new Tab). Bare URLs without brackets are not links in the parser and stay text.

After trying it: every link follows on ⌘-click only, a plain click places the cursor; ⌥⌘-click opens a Wikilink in a new Tab.
