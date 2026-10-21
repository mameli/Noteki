# Noteki starts from scratch and borrows MIT code, it is not a fork of MarkEdit

MarkEdit has the same architecture as Noteki ([ADR 0001](0001-codemirror-in-wkwebview.md)) and has already solved most of the WKWebView pitfalls, but it is a document-based app: one file per window, no Vault, no sidebar, no index, plus many features Noteki does not want (Writing Tools, Translation, Quick Look). What makes Noteki useful is exactly what MarkEdit lacks, and a fork would mean keeping up with someone else's codebase to use a small part of it. Noteki is written from scratch and copies, adapting them, the pieces it needs from MIT projects: from MarkEdit the web view handling (find, select-all, word movement, key handling), from SilverBullet and MarkEdit-preview the CodeMirror extensions for Live preview. codemirror-markdown-tables is a dependency for tables.

## Consequences

- Every borrowed piece is listed in `THIRD_PARTY.md` with its project, licence and what was changed.
- Zettlr's Live preview is the most complete but is GPL-3.0, and Obsidian's is closed: they can be looked at, never copied.
