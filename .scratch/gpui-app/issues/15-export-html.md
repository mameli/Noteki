# 15: Export as HTML

**What to build:** *Export as HTML…* writes the Note in front as one page standing alone, as the Reader draws it (the `markdown` crate the Reader uses), light colours, images of the Vault put in as data, the Frontmatter and Comments left out. SPEC › Export (PDF and print need WebKit: out of scope).

**Blocked by:** none

**Status:** done

- [x] One self-contained file; links to Notes as text
- [x] Tests: the HTML of the test note without GPUI

## Comments

- `core::export::note_html` builds the page from the Reader's preprocessing and the same `markdown` crate (GFM). The Frontmatter and Comments, inline and across lines, are left out. A line break is kept as one. Wikilinks become `<span class="wikilink">` with their name or Alias. The Vault's images, embedded or by relative path, become `data:` addresses. The CSS is light and fixed.
- `javascript:`/`vbscript:`/`data:` link addresses are emptied: our own schemes need `allow_dangerous_protocol`, so we neutralize them ourselves (mutant-checked).
- "Export as HTML…" is in the Command palette. The save dialog opens in Downloads with `Note.html`; nothing is written in the Vault unless chosen there. Not in a Loose file's window (YAGNI).
- Math stays as written and Mermaid as code (no KaTeX/Mermaid without WebKit). PDF and Print are out of scope.
