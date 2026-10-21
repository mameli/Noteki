# The editor is CodeMirror 6 in a WKWebView, the rest is native Swift

Noteki exists to be lighter than Obsidian (441 MB on disk, Electron) and to keep its **Live preview**, where Markdown syntax hides outside the cursor. Doing Live preview in TextKit is the hard path: FSNotes, the most mature native open source alternative, hides syntax with a 0.1 pt clear font and only for the whole Note at once, never around the cursor; MiaoYan's author calls WYSIWYG "heavy and fragile" and keeps a separate preview. Tables (in 108 of the 624 Notes of the Vault), images and checkboxes inside a TextKit editor are the hardest part. CodeMirror 6 is the engine Obsidian itself uses for Live preview, and MIT code to hide syntax, show tables and images already exists (SilverBullet, MarkEdit-preview, codemirror-markdown-tables). A WKWebView uses the system's WebKit, so unlike Electron nothing big ships with the app: MarkEdit, a native Mac editor built exactly this way, is 3.2 MB on Apple Silicon.

Only the Note being edited lives in the web view. The Vault, the index, Wikilink resolution, search, Merge, the sidebar, images and PDFs (PDFKit) are Swift, so the JavaScript side stays small and receives data, not responsibilities. The bridge between the two is a typed message enum on each side, written by hand: about ten messages do not justify MarkEdit's code generation (ts-gyb).

## Considered options

- **Live preview in TextKit 2**: fully native and smallest, but months of work on cursor movement over hidden characters, and tables and images would still need a separate reading view.
- **Separate reading and editing views** (FSNotes, MiaoYan, iA Writer): simpler, but not what the user wants to use every day.
- **Tauri or Electron**: a web app with a native look at best; Electron is the weight Noteki is escaping.

## Consequences

- Native text behaviour must be wired by hand: CodeMirror draws only the visible lines, so select-all, find, Services and accessibility do not work by themselves. MarkEdit's solutions (MIT) are the reference.
- Spellcheck, autocorrect and input methods come from WebKit through the `spellcheck` and `autocorrect` attributes.
- Each WKWebView costs a WebKit process of roughly 40–60 MB, which shapes how windows and Tabs work ([ADR 0003](0003-one-web-view-per-window.md)).
