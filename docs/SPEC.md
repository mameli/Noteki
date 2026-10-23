# Noteki: v1 specification

Terms in **bold** are defined in [CONTEXT.md](../CONTEXT.md). Architecture decisions are in [docs/adr](adr/).

## Goal

Read and write the Markdown **Vault** the user keeps with Obsidian, in a native Mac app a fraction of its weight, keeping the parts of Obsidian used every day: **Live preview**, **Frontmatter**, **Tags**, **Wikilinks**, **Backlinks**. The first use is reading and editing the Meeting notes Takku writes, during and after a Meeting.

Personal use, signed with the user's Developer ID, not sandboxed, not on the App Store. macOS 15 or later, Apple Silicon.

## Budget

Measured at the end of every milestone on a generated Vault of 5,000 Notes (see [Testing](#testing-and-measurement)). Obsidian, for comparison, is 441 MB on disk.

| | Limit |
|---|---|
| App on disk | < 20 MB |
| Cold start to the last Note shown | < 1 s |
| Index of 5,000 Notes | < 3 s, the window usable before it ends |
| Memory at rest, one window, one Note | < 150 MB, all processes included |

A feature that breaks the budget waits until it fits.

Memory is measured as at first launch: the right sidebar hidden, the left one on Files. Showing the right sidebar costs about 10 MB of Noteki's own process, on its Outline, and its Backlinks pane about 6 MB more, the links of a Vault of 5,000 Notes (8 bytes a link) and their list; showing the Tags pane about 1 to 5 MB (CHANGELOG). The index of 5,000 Notes counts the Files listing and, with a pane shown, the reading of what it needs (links, Tags). WebKit's processes vary between runs, from 50 to 90 MB in all, so a total is only comparable with another taken in the same conditions; Noteki's own process is steady.

## Scope

**In v1**: everything in this document.

**Right after v1**: renaming or moving a Note updates the Wikilinks that point to it (Obsidian's *Always update internal links*, on in the user's Vault).

**Not before someone needs it**: Mermaid diagrams (shown as code), math, links to a Heading or a block (`[[note#heading]]`, `[[note^id]]`, unused in the Vault today), embedding a Note in another, more Vaults at once, a command palette, a persistent index ([ADR 0006](adr/0006-in-memory-index.md)).

**Never**: graph view, canvas, plugins, themes and CSS snippets, sync, git, publishing, an iOS app. Obsidian stays available on the same Vault for these ([ADR 0002](adr/0002-coexist-with-obsidian.md)).

## Vault

- At first launch Noteki asks for the Vault folder and remembers it. *File → Open Vault…* changes it. One Vault at a time.
- Noteki writes nothing in the Vault but Notes and Attachments ([ADR 0002](adr/0002-coexist-with-obsidian.md)). Settings live outside the Vault, in the app's preferences and in `~/Library/Application Support/Noteki/`.
- **Listed and indexed**: `.md` Notes; images (`png`, `jpg`, `jpeg`, `gif`, `webp`, `svg`, `heic`) and `pdf` as **Attachments**. Every other file is ignored, in the sidebar too.
- **Excluded folders**: every folder whose name starts with `.`; `node_modules`, `.venv`, `venv`, `__pycache__`; every folder that contains a `.git` of its own (a repository inside the Vault), the Vault's own root excepted; the folders the user adds in *Settings*. In the user's Vault this leaves out about 13,000 SVG files and the code repositories under `Projects/` and `Sandbox/`.

## Window

- **Left sidebar**, three panes: *Files* (the folder tree), *Tags* (the Tag tree with counts, click to list the Notes), *Search*.
- **Centre**: the Note of the Tab in front, with the tab bar above it.
- **Right sidebar**, collapsible: *Backlinks* of the Note (with the line around each one) and its **Outline**.
- **Tabs** are Noteki's own, one web view per window ([ADR 0003](adr/0003-one-web-view-per-window.md)). Clicking a Note in the sidebar opens it in the Tab in front; ⌘-click opens it in a new Tab. The open Tabs of each window are restored at launch.
- Images and PDFs open in the centre in a native view (an image view, PDFKit), in a Tab like a Note.
- Appearance follows the system light or dark mode. One theme. *Settings* chooses the text font, its size and the maximum line width.

## Notes

### Live preview

The Note is shown formatted, and the Markdown syntax of an element appears when the cursor is inside it ([ADR 0001](adr/0001-codemirror-in-wkwebview.md)):

- **Inline elements** (bold, italic, `==highlight==`, inline code, links, Wikilinks, `#tags`): the marks appear when the cursor is inside that element, not on the whole line.
- **Blocks** (tables, code blocks, the Frontmatter, callouts): rendered while the cursor is outside, shown as source when it enters.
- Showing or hiding syntax never changes the height of a line above the cursor: the text must not jump.

Rendered in v1: Headings, emphasis, highlights, lists, task lists (the checkbox is clickable and writes `[x]` in the file), quotes, callouts (`> [!note]`, title and colour, no folding), tables (via codemirror-markdown-tables), code blocks with syntax colouring, horizontal rules, links, Wikilinks, Embeds of images. **Comments** (`%% … %%`) are hidden outside the cursor. Mermaid and math stay visible as code.

⌘E switches the Tab between Live preview and **Source mode**.

### Frontmatter

- Shown as a two-column *Properties* table above the text: lists one item per line, values as written.
- Clicking the table turns it into the YAML source for editing, back to the table when the cursor leaves.
- The `tags` row is the exception: Tags are chips, with a field to add one (completed from the Tags of the Vault) and a × to remove one. Noteki writes them back as a YAML list.
- YAML Noteki cannot read is shown as source with an error mark, never rewritten.

### Saving

- Noteki saves by itself about one second after the last change, and when the Tab or the app closes. There is no *Save*.
- It writes the whole file atomically and keeps line endings and the final newline as they were.
- A Note larger than 5 MB opens in Source mode.

### External changes

Noteki watches the Vault with FSEvents ([ADR 0005](adr/0005-merge-external-changes.md)):

- **Changed, nothing unsaved**: reloaded silently, the cursor kept at the same place in the text.
- **Changed with unsaved edits**: three-way **Merge** with the last version Noteki saved or loaded as base, then saved.
- **Conflict**: the editor keeps the user's text and stops saving that Note; a banner offers *Keep mine*, *Take theirs* and *Show differences*.
- **Renamed or moved** (same file identity, new path), as Takku does after Processing: the Tab follows it (**Followed rename**).
- **Replaced** (new file identity at the same path), as an atomic write by Takku or another editor does: treated as *changed*, not as deleted and created.
- **Deleted**: the Tab stays open with the text in memory and a banner offers to save it again.

## Links

### Wikilinks

- `[[name]]` and `[[name|alias]]` are links: click (or ⌘-click for a new Tab) opens the Note.
- **Resolution**, exactly as Obsidian 1.14.4's `getLinkpathDest`, read from its code: by file name in any case, `.md` optional; a path from the Vault's root wins when it matches exactly, `./` and `../` are relative to the linking Note's folder, and otherwise a path also matches the end of a longer one (`Axo/Retro` finds `Clients/Axo/Retro.md`). Among files with the same name, those whose path starts with the linking Note's folder come first, then the shortest path. `[[#heading]]` leads to the Note itself. Only files Noteki lists can be found: not those in Excluded folders, nor Obsidian's `.canvas`.
- An **Unresolved link** is drawn faded, as in Obsidian; so is an Embed of a missing file. Following it creates an empty Note and opens it: a bare name in the same folder as the Note that holds the link (Obsidian's "same folder as current file"; its default is the Vault's root), a path from the Vault's root as Obsidian does, `./` and `../` from the linking Note's folder. Never over an existing file, never for an Attachment's name, nor with a character Obsidian refuses on a Mac (`\` or `:`).
- Typing `[[` opens a completion list of the Notes, fuzzy on the name, the most recently opened first among names that match equally well. Choosing one writes its name, or the shortest path that is unique when the name is ambiguous (Obsidian writes the whole path; the shorter one leads to the same Note under its resolution), and closes the brackets. Inside a link already closed it replaces the name and keeps the Alias or heading. Not in code. The recently opened Notes are remembered per Vault in the app's preferences.

### Embeds and Attachments

- `![[image.png]]` and `![](image.png)` show the image in the Note, as wide as the text at most.
- `![[file.pdf]]` is shown as a link to the PDF, which opens in a Tab.
- **Pasted image**: pasting an image saves it as `Pasted image YYYYMMDDHHmmss.png` in the Note's folder and writes `![[Pasted image YYYYMMDDHHmmss.png]]` at the cursor. Dropping an image file from the Finder copies it next to the Note the same way, keeping its name.

### Backlinks

The right sidebar lists the Notes with a Wikilink to the Note in front, each with the line that holds the link (cut around the link when it is long). Clicking one opens that Note with the cursor on that line. Unlinked mentions (the name written without `[[ ]]`) are not listed.

- A link counts as Obsidian counts it: `[[name]]`, with an Alias, a heading or a block (`[[name#heading]]`, which leads to the Note), a path, an Embed `![[name]]`, and a link in the Frontmatter. A link to the Note itself does not, and several on a line are one Backlink. Which Note a link leads to is decided by the same resolution as a click, from the Note that holds it.
- Links in code (inline, fenced) do not count; a backslash-escaped `\[[` does not either. Not excluded: indented code blocks, fences nested in a list, a code span across lines, `%%comments%%`.
- Backlinks come from the files on disk: a link typed in a Note shows in the Backlinks of its target a moment after it is saved.
- The sidebar is **hidden at first** (⌥⌘0, or the toolbar button; remembered in the preferences). Its *Outline* and *Backlinks* are chosen with a control at its top, the Outline first; the choice is remembered too. The links of the Vault are read when the Backlinks are first shown, not at launch, because holding them costs memory (see Budget). Until then, "Reading the links of your Notes…".

### Outline

The right sidebar's Outline lists the Headings of the Note in front, indented by level, as it is typed: `#` to `######`, and a paragraph underlined with `===` or `---` (at its first line; a rule under a list item, a quote or another rule is no Heading). Not in fenced code nor in the Frontmatter; Headings inside a quote, a list item or a `%%` Comment are not distinguished yet. A Heading's text is shown without the marks of its links, emphasis and code; an empty Heading is left out. A click puts the cursor at the start of the Heading's line, in the middle of the view.

## Tags

- Read from the `tags` Property (list or single value, with or without `#`) and from `#tag` in the text, outside code. A Tag starts with a letter and may contain letters, digits, `_`, `-` and `/`.
- `client/axo` is nested: the *Tags* pane shows `client` with `axo` under it, and choosing `client` lists the Notes of both. Each Tag shows the number of Notes that have it or a Tag under it, each Note counted once.
- A `#tag` in the text follows the start of a line or a space, as in Obsidian: `(#tag)` and `word#tag` are not Tags. The Frontmatter is not read for them. `#`, `\#` and the text of code (inline, fenced) are not Tags. Letters are any letters, accented or in other scripts.
- Digits are 0–9 and those of other scripts, not `²` or `½`. A Tag that is only digits (`#2024`) is not one, as it starts with no letter.
- Upper and lower case are one Tag: `#Client` and `#client`, and so is the Tag chosen in the pane and, from ticket 17, in `tag:`. It is shown as most Notes write it, the lowercase form between equals.
- Not excluded as code, as for Backlinks: indented code blocks, fences nested in a list, a code span across lines, `%%comments%%` and HTML (`style="color: #fff"` is the Tag `fff`).
- In the Frontmatter only the `tags` key is read: a block list (blank lines allowed in it), a flow list (on one line or several), or one value, or several separated by commas or spaces; quotes and `#` are dropped, and a YAML comment (`alpha # old`) is not read. A value that is not a Tag, or `null`, is skipped.
- The Tags are read when the *Tags* pane is first shown, not at launch (the memory of the Budget, as with the Backlinks); until then "Reading the Tags of your Notes…". They follow the files on disk: a Tag typed in a Note is in the pane a moment after it is saved.
- The left sidebar's *Files*, *Tags* and *Search* are chosen with a control at its top; the choice is remembered in the preferences.
- Noteki adds and removes Tags only in the `tags` Property, never in the text.

## Search and navigation

- **Quick switcher** (⌘O): fuzzy on Note names, ranked by how well the name matches (the name itself, then a start, a word's start, inside, letters in order), and most recently opened first among names that match equally well; with nothing typed, the recent Notes first. A path or a name with `.md` finds its Note as a Wikilink would. Enter opens in the Tab in front, ⌘Enter in a new Tab (with Tabs, ticket 25). A name that exists nowhere offers to create the Note, where a Wikilink's would be created from the Note or Attachment in front; not a name the Vault or a Wikilink refuses. The editor takes the keyboard once the Note is in front; Escape gives it back where it was.
- **Search** (⌘⇧F): words in any order, all required, case and accents ignored (`strasse` finds `Straße`). A word may be in the Note's name or anywhere in its text, the Frontmatter and code included. `tag:client/axo` keeps the Notes with that Tag or one under it, with the Tag rules of the Tags pane; `path:Meetings/` keeps those whose path holds it, anywhere in the path. Double quotes keep spaces: `"late order"`, `path:"Folder 1/"`. Results grouped by Note: those whose name holds every word first, then by path, each with the lines that hold a word (ten at most, then a count). The first 300 Notes are listed, then a count. A click on a Note opens it, on a line opens it there, also in the Note already in front.
- Search reads the files each time, about 0.2 s for 5,000 Notes once macOS has them in its cache, and keeps nothing (ADR 0006). It follows them on disk: results are found again a moment after a file changes, so typing is found after the autosave.
- **New Note** (⌘N): `Untitled.md` (then `Untitled 1.md`…) in the folder of the Note in front, its name selected to type over.
- **Daily note** (*File → Today's Note*, ⌥⌘D): folder and date format in *Settings*, `Journal/Daily_2025` and `YYYY-MM-DD` to match the user's Vault.
- In the *Files* pane, Notes and folders can be created, renamed, moved by dragging and moved to the Trash (never deleted for good). In v1 renaming does not update the Wikilinks that point to the Note.
- Other shortcuts follow Obsidian: ⌘W close Tab, ⌃Tab / ⌃⇧Tab next and previous Tab, ⌘F find in the Note, ⌘⌥← / ⌘⌥→ back and forward.

## Takku

Noteki works with Takku without changes to Takku:

- Its `Info.plist` declares the *Editor* role for `net.daringfireball.markdown`, so Takku lists it among the Editors, and the *Viewer* role for `public.folder`, so Takku's *Show all* hands it the Vault. Not `public.plain-text`: that type covers every source file.
- Opening a Note or Attachment of the Vault shows it; a folder equal to or inside the Vault is selected in *Files*; a file in an Excluded folder only brings the window forward. A Markdown or text file outside the Vault opens as a **Loose file** in its own window, without indexing anything around it. Any other folder asks whether to switch Vault.
- During Processing Takku writes the Meeting note several times and renames it: [External changes](#external-changes) covers both.

## Project structure

- `NotekiCore/`: Swift package, no AppKit. Vault scanning and Excluded folders, Frontmatter parsing, Tags, Wikilink resolution, the index, search, three-way Merge. Everything here is unit tested.
- `Noteki/`: the SwiftUI + AppKit app. Windows, sidebars, Tabs, the editor's web view and its bridge, file watching, image and PDF views, settings.
- `Editor/`: TypeScript, CodeMirror 6 and the Live preview extensions, built into one `editor.html` committed in the app's resources, with a hash of its sources that the build checks ([ADR 0004](adr/0004-committed-editor-bundle.md)).
- The bridge between `Noteki` and `Editor` is a typed message enum on each side, written by hand ([ADR 0001](adr/0001-codemirror-in-wkwebview.md)). A test checks that the names match.
- `THIRD_PARTY.md`: every piece of code borrowed from MarkEdit, SilverBullet, MarkEdit-preview and others, with licence and changes ([ADR 0007](adr/0007-from-scratch-borrowing-mit-code.md)).

## Testing and measurement

- Swift Testing in `NotekiCore` on a fixture Vault in the repository, written for the purpose: duplicate names, aliases, Embeds, nested Tags, broken YAML, Excluded folders. Never the user's real Vault.
- Merge tests replay the sequence of writes Takku makes during Processing (atomic writes, rename) against a user still typing.
- A few JavaScript tests for the Live preview extensions: what is hidden where the cursor is.
- `scripts/measure`: generates a Vault of 5,000 Notes and measures the [Budget](#budget). Run at the end of every milestone, results noted in the changelog.

## Milestones

Each milestone ends with an app the user can use every day.

1. **M1, read and write**: open the Vault, *Files* pane, CodeMirror in Source mode with syntax colouring, saving, External changes with Merge, Conflict and Followed rename, images and PDFs in native views, the Takku document types.
2. **M2, the Vault**: index, Wikilinks clickable and completed, Quick switcher, Backlinks, Tags pane, Search, Outline.
3. **M3, Live preview**: hidden syntax, tables, images in the text, task lists, callouts, Comments, the Properties table with Tag chips.
4. **M4, the rest**: Tabs, Pasted images and dropped files, Daily note, file operations in *Files*, settings.

## Known risks

- **Wikilink resolution with duplicate names**: Obsidian's rule is undocumented. It was read from Obsidian 1.14.4's code and written as tests (ticket 11), but not checked by hand in Obsidian, and a later Obsidian may change it. Ties in path length keep Files' order, where Obsidian keeps its own loading order.
- **Native text behaviour in the web view**: select-all, find, Services and accessibility do not work by themselves because CodeMirror draws only the visible lines. MarkEdit's solutions are the starting point.
- **Live preview that jumps**: hiding and showing syntax changes widths and heights. Every extension is tried on the Vault's longest Meeting notes before M3 ends.
- **Merge on large rewrites**: when Takku rewrites most of a Note at once, a Conflict may be frequent. If it is, the base is taken at the moment the user starts typing rather than at the last save.
- **The memory budget**: the WebKit processes alone use about 80 MB of the 150. Measured at M1, before anything else is built on it, and at the end of every milestone. At M2 the budget holds only with the right sidebar hidden; what Tags, Search and the Outline add has to fit in what is left, or be read only when its pane is first shown, as the Backlinks are.
