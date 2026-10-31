# 34: A cared-for find bar, ⌘D for the next match

**What to build:** The in-Note find bar (⌘F) is CodeMirror's default panel, unstyled: grey bulky buttons, lowercase labels, plain checkboxes. Redraw it to look native on macOS, like Xcode's or Safari's find bar, and add ⌘D.

**Status:** ready-for-agent

- [x] One compact row: search field with a magnifier, match count ("3 of 12"), previous/next as chevron buttons, Done
- [x] Match case, regexp and whole word as small toggle icons (Aa, .*, ab) inside or beside the field
- [x] Replace row shown only on demand (a disclosure; no ⌥⌘F yet), with Replace / All
- [x] System font and colours, light and dark, following the editor's theme
- [x] Return / ⇧Return go to next / previous, Esc closes
- [x] ⌘D adds the next occurrence of the selection to the selections (multi-cursor)

## Comments

From user feedback 2026-10-31: "Migliorare la ricerca. Voglio anche cmd + d per cercare le parole uguali"; then, with a screenshot of the current panel: "il menù di ricerca è semplicemente poco curato". Search across the Vault is not in scope.

Done. `findbar.ts` replaces CodeMirror's panel with its own, on the same search state (⌘G, ⇧⌘G). Return / ⇧Return, ⌘Return in Replace for All, Esc closes. ⌘D was in CodeMirror's keymap but did nothing: multiple selections are now allowed and drawn by CodeMirror (`drawSelection`). Not checked by hand in the app.
