# NotekiGPUI: the app around the editor

Status: ready-for-agent

Terms in **bold** are defined in `CONTEXT.md`; behaviour is in `docs/SPEC.md`. The GPUI prototype only (`NotekiGPUI/`), free to change (the Swift app on `main` is the stable Noteki). The editor itself is `.scratch/gpui-editor/`.

## Problem Statement

The new editor draws every element of a Note, but the window around it is still the first prototype's: files are renamed, copied and trashed only from three buttons that act on the Note in front, a right-click does nothing, a Tab closes only from its ×, the mouse's back and forward buttons do nothing. Replacing GPUI Kit's editor also lost find (⌘F). The user's feedback notes in their Vault (`Noteki Feedbacks.md`, `Noteki Feedbacks 2.md`, 12 November 2026) ask for the same things, and more:

- "Quando apro un file con il tasto destro, voglio che il menu sia più ricco. Ad esempio, voglio che ci sia la possibilità di fare una copia del path al file."
- "Voglio che nel menù dei file ci sia anche un clona file per creare una copia oltre quindi a new note, rename etc"
- "Voglio poter chiudere le tab con il tasto centrale del mouse"
- "Migliorare la ricerca. Voglio anche cmd + d per cercare le parole uguali" (with a screenshot of CodeMirror's find panel: next, previous, all, match case, regexp, by word, replace, replace all)
- "Non si può ingrandire il font con cmd +"
- "Quando seleziono un codice tipo `asiodfjoiasj` è praticamente invisibile."

Already done in the prototype, from the same notes: links open on click, a click on a folder opens it, images load, `---` is drawn.

## Solution

Five small pieces, each with GPUI Kit's own components (`Tree::context_menu`, `ContextMenuExt`, `PopupMenu`) and crates already built in (`regex`):

1. Selected text stays visible over inline code and highlights.
2. Find and replace in the Note (⌘F), and ⌘D for the next occurrence of the selection.
3. Text size with ⌘+, ⌘− and ⌘0, remembered.
4. A context menu on files and folders, and on Tabs; a middle click closes a Tab.
5. Back and forward with the mouse's side buttons, and with ⌘⌥← / ⌘⌥→ wherever the focus is.

## User Stories

1. As the user, I want selected inline code to show as selected, so that I see what I copy.
2. As the user, I want ⌘F to find in the Note, with next, previous, all, match case, regular expressions and whole words, and replace and replace all, so that I find and fix text as in Obsidian.
3. As the user, I want ⌘D to select the next occurrence of the word or selection, again and again, and to type over all of them at once, so that I rename a word everywhere quickly.
4. As the user, I want ⌘+ and ⌘− to make the text larger and smaller and ⌘0 to reset it, remembered at the next launch, so that I read comfortably.
5. As the user, I want a right-click on a file to open it, open it in a new Tab, rename it, make a copy, copy its path, reveal it in the Finder and move it to the Trash, so that I manage files where I see them.
6. As the user, I want a right-click on a folder to create a Note or a folder in it, rename it, copy its path, reveal it and trash it, so that I organize the Vault from the sidebar.
7. As the user, I want a middle click on a Tab to close it, and a right-click to close it, the others or those to its right, copy its path and reveal it, so that I manage Tabs as in a browser.
8. As the user, I want the mouse's back and forward buttons to go back and forward, so that I move through the Notes I followed.

## Implementation Decisions

- The rules stay in `core` and are tested there: what a find matches (plain, case, whole word, regex) in `core::editor::find`; the multiple selections of ⌘D in `EditBuffer`; which Tabs "close others" and "close to the right" close in `core::tabs`.
- Menus are GPUI Kit's `PopupMenu` built per right-click, items acting through `NotekiApp` methods that already exist (`open_path`, `duplicate`, `trash`, the name prompt) or small new ones (copy path, reveal in Finder with `cx.reveal_path`).
- Find highlights all matches as a background on the lines, through the same per-line styles as Live preview, so that it costs a line's restyle, not the Note's, when the cursor moves.
- The text size is a setting (`font_size`, 10–32 points as SPEC › Window says, 15 by default) applied to the editor; ⌘0 puts the default back.

## Testing Decisions

As in `.scratch/gpui-editor`: rules tested without GPUI, UI tests through real key and mouse events on the real `NotekiApp`, a mutant per fixed behaviour, keystroke timing kept.

## Out of Scope

The Properties table (the prototype shows the Frontmatter as YAML), the format bar of a Loose file (the prototype has no Loose file), drag and drop of files and Tabs.

## Second batch: what the prototype still lacks of SPEC v1 (tickets 07–16)

After tickets 01–06 and the editor's batch, what SPEC v1 has and the prototype does not, in the order it matters for using the prototype every day. Asked by the user on 17 November 2026 ("sono tutte cose che possono essere utili"); the package's size "non è bloccante" (done: 29 → 17 MB with strip and fat LTO, same speed).

- **07** `[[` completion while typing.
- **08** Callouts, Comments, footnotes and setext Headings in Live preview.
- **09** The Properties table over the Frontmatter, Tags as chips.
- **10** Pasted and dropped images saved next to the Note.
- **11** Small editor gaps: ⌘K link, ⌘/ Comment, a table cell's ellipsis, indented code blocks, math left as written.
- **12** A real `.app`: bundle, icon, opening files from the Finder and Takku, Loose files.
- **13** Settings: font, line length, Excluded folders, Daily note.
- **14** A second window (⇧⌘N).
- **15** Export as HTML.

Out of scope for now, each needs native code GPUI does not offer: pasting formatted text as Markdown (GPUI's clipboard gives text, images and files, not HTML; it would take reading `NSPasteboard` directly and an HTML-to-Markdown converter), spelling and autocorrect (`NSSpellChecker`), PDFs in a native view (PDFKit; a PDF opens in the default app), print and export as PDF (WebKit's).
