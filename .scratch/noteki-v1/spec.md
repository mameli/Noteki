# Noteki v1

Status: ready-for-agent

Terms in **bold** are defined in `CONTEXT.md`. The detailed behaviour is in `docs/SPEC.md`; the architecture decisions are in `docs/adr/` (ADR 0001–0007). This spec does not override them.

## Problem Statement

I keep my notes in a Markdown **Vault** that I open with Obsidian, and Takku writes my Meeting notes and Transcripts into it. I like Obsidian, but it is an Electron app of 441 MB on disk that is slow to start and heavy at rest, and most of it (plugins, graph, canvas, sync) I never use. What I do every day is small: read and write Notes, see their **Frontmatter**, tag them, follow **Wikilinks**, look at images and PDFs, and keep writing in a Meeting note while Takku rewrites it. I want an app that does only that, natively, without making me leave Obsidian behind for the few things only it does.

## Solution

Noteki is a native macOS app that opens the same Vault as Obsidian and leaves it as Obsidian expects. It shows each Note in **Live preview**: Markdown formatted in place, with its syntax visible only where the cursor is, so the same view is for reading and writing. It reads the Frontmatter as a table of **Properties**, tags Notes, resolves Wikilinks and **Backlinks** with Obsidian's rules, searches the Vault and opens images and PDFs. It saves by itself and **Merges** **External changes**, so Takku and Obsidian can write the same Note while I am in it. It stays under 20 MB on disk and 180 MB of memory at rest.

## User Stories

### Opening the Vault

1. As the user, I want to choose my Vault folder at first launch, so that Noteki knows where my Notes are.
2. As the user, I want Noteki to reopen the same Vault at every launch, so that I never choose it again.
3. As the user, I want to switch to another Vault from the File menu, so that I can open a different folder when I need it.
4. As the user, I want Noteki to open my existing Obsidian Vault as it is, so that I do not migrate anything.
5. As the user, I want Noteki never to write in `.obsidian/` or add its own files to the Vault, so that Obsidian keeps working on the same folder.
6. As the user, I want Noteki's settings stored outside the Vault, so that they are not synced or committed with my Notes.
7. As the user, I want only Notes, images and PDFs listed, so that code, data and other files do not clutter the sidebar.
8. As the user, I want hidden folders, `node_modules`, virtual environments, `__pycache__` and repositories inside the Vault excluded, so that the thousands of files in my code folders are neither listed nor indexed.
9. As the user, I want to add my own **Excluded folders** in Settings, so that I can hide any other folder I never read.
10. As the user, I want the window usable while the Vault is still being indexed, so that a big Vault never makes me wait to read a Note.

### Window and Tabs

11. As the user, I want a sidebar with my folders and Notes, so that I can browse the Vault.
12. As the user, I want a sidebar pane with all my **Tags** as a tree, so that I can find Notes by Tag.
13. As the user, I want a sidebar pane for search, so that I can find Notes by their text.
14. As the user, I want a collapsible right sidebar with the Backlinks and the **Outline** of the Note in front, so that I see its context without leaving it.
15. As the user, I want to open more Notes in **Tabs** in the same window, so that I can move between them quickly.
16. As the user, I want each Tab to keep its own cursor, scroll and undo, so that switching Tab never loses my place.
17. As the user, I want ⌘-click on a Note or a link to open it in a new Tab, so that I keep the current one.
18. As the user, I want ⌘W, ⌃Tab and ⌃⇧Tab to close and switch Tabs, so that I use the shortcuts I know from Obsidian.
19. As the user, I want back and forward (⌘⌥← / ⌘⌥→), so that I can return to the Note I came from after following a link.
20. As the user, I want the Tabs of each window restored at launch, so that I continue where I left off.
21. As the user, I want to open a second window (⌘⇧N), so that I can look at two Notes side by side.
22. As the user, I want Noteki to follow the system light or dark mode, so that it matches the rest of my Mac.
23. As the user, I want to choose the font, its size and the maximum line width, so that long Notes are comfortable to read.

### Reading and writing a Note

24. As the user, I want a Note shown formatted, with Headings, emphasis, lists and quotes rendered, so that I read it as a document and not as code.
25. As the user, I want the syntax of bold, italic, highlights, inline code, links, Wikilinks and Tags to appear only when my cursor is inside that element, so that I can edit it without the rest of the line changing.
26. As the user, I want tables rendered as tables and shown as source when my cursor enters them, so that my many tables are readable and still editable.
27. As the user, I want code blocks with syntax colouring, so that code in my Notes is readable.
28. As the user, I want task list checkboxes I can click, so that I tick a task without editing the text by hand.
29. As the user, I want callouts shown with their title and colour, so that they look as they do in Obsidian.
30. As the user, I want **Comments** (`%% … %%`) hidden unless my cursor is in them, so that Takku's markers do not clutter my Meeting notes.
31. As the user, I want the text never to jump when syntax appears or disappears, so that reading and typing stay calm.
32. As the user, I want to switch a Note to **Source mode** with ⌘E, so that I can see and edit every character as written.
33. As the user, I want Mermaid and math shown as code, so that they remain readable even if not rendered.
34. As the user, I want spellcheck, autocorrect and input methods to work as in any Mac app, so that writing feels native.
35. As the user, I want ⌘F to find in the Note, select-all to select the whole Note and copy and paste to behave normally, so that the editor never surprises me.
36. As the user, I want a very large Note to open in Source mode, so that it opens quickly.

### Frontmatter and Tags

37. As the user, I want the Frontmatter shown as a table of Properties above the text, so that I can read Takku's `participants`, `duration` or `speakers` at a glance.
38. As the user, I want lists in the Properties table shown one item per line, so that long lists stay readable.
39. As the user, I want to click the Properties table to edit the YAML, so that I can change any Property as text.
40. As the user, I want Frontmatter Noteki cannot read shown as source with an error mark and never rewritten, so that a typo of mine is never made worse.
41. As the user, I want the `tags` Property shown as chips, so that I see a Note's Tags at once.
42. As the user, I want to add a Tag to a Note from a field completed with the Tags of the Vault, so that I tag Notes consistently.
43. As the user, I want to remove a Tag with one click, so that tagging is quick.
44. As the user, I want Tags added by Noteki written only in the `tags` Property, so that my Notes keep the format I already use.
45. As the user, I want `#tags` written in the text recognised too, outside code, so that older Notes are tagged as well.
46. As the user, I want nested Tags (`client/axo`) shown as a tree, so that choosing `client` lists the Notes of all its sub-Tags.
47. As the user, I want each Tag in the Tags pane to show how many Notes have it, so that I see how my Vault is organised.

### Wikilinks, Embeds and Backlinks

48. As the user, I want `[[name]]` and `[[name|alias]]` to be links I can click, so that I move between Notes as in Obsidian.
49. As the user, I want Wikilinks resolved with Obsidian's rules, so that a link leads to the same Note in both apps.
50. As the user, I want **Unresolved links** drawn in another colour, so that I see which Notes do not exist yet.
51. As the user, I want following an Unresolved link to create the Note in the folder of the Note I am in and open it, so that new Notes land next to their context.
52. As the user, I want a completion list when I type `[[`, so that I link Notes without remembering their exact names.
53. As the user, I want the completion to write the shortest unique path when two Notes share a name, so that the link is never ambiguous.
54. As the user, I want `![[image.png]]` and `![](image.png)` to show the image in the Note, so that my screenshots and pasted images are visible where they belong.
55. As the user, I want `![[file.pdf]]` shown as a link that opens the PDF, so that I reach the document from the Note.
56. As the user, I want to see the Backlinks of a Note with the line around each, so that I know where it is referenced and why.

### Navigation and search

57. As the user, I want a **Quick switcher** (⌘O) that finds Notes by part of their name, so that I open any Note in a second.
58. As the user, I want the Quick switcher to put the Notes I opened recently first, so that I reach my current work faster.
59. As the user, I want the Quick switcher to offer creating a Note whose name matches nothing, so that I create Notes without leaving the keyboard.
60. As the user, I want to search the Vault (⌘⇧F) for words in any order, ignoring case and accents, so that I find a Note even when I misremember its wording.
61. As the user, I want `tag:` and `path:` in the search, so that I can narrow the results to a client or a folder.
62. As the user, I want results grouped by Note, with matches in the name first and the matching lines shown, so that I pick the right one quickly.
63. As the user, I want the Outline of a Note to jump to a Heading, so that I move around long Meeting notes.

### Files

64. As the user, I want to create a Note (⌘N) in the folder of the Note in front, with its name ready to type, so that new Notes land where I am working.
65. As the user, I want to create, rename and move Notes and folders from the sidebar, so that I organise the Vault without the Finder.
66. As the user, I want deleting a Note to move it to the macOS Trash, so that nothing is ever lost by mistake.
67. As the user, I want to paste an image into a Note and have it saved next to the Note as a **Pasted image** with an Embed at the cursor, so that screenshots go into my Notes as they do in Obsidian.
68. As the user, I want to drop an image file from the Finder into a Note and have it copied next to the Note, so that I add images without managing files.
69. As the user, I want to open images and PDFs from the sidebar in a native view, in a Tab, so that I read them without another app.
70. As the user, I want a **Daily note** command (⌥⌘D) that opens today's Note in my Journal folder, creating it if missing, so that I keep my journal without the journals plugin.
71. As the user, I want to choose the Daily note's folder and date format, so that it matches the Daily notes I already have.

### Saving and External changes

72. As the user, I want Noteki to save by itself shortly after I stop typing, so that I never think about saving.
73. As the user, I want saving to keep line endings and the final newline as they were, so that Noteki never produces noise in my Vault's git history.
74. As the user, I want a Note changed by another app while I am not typing to reload silently with my cursor kept in place, so that I always see the current text.
75. As the user, I want my unsaved edits Merged with an External change, so that neither my notes nor Takku's Summary is lost when we write at the same time.
76. As the user, I want a **Conflict** to keep my text, stop saving that Note and show me a banner with the differences, so that I decide what to keep.
77. As the user, I want to choose *Keep mine* or *Take theirs* from the Conflict banner, so that I resolve it in one click when the answer is obvious.
78. As the user, I want a Note renamed or moved by another app while open to stay open under its new name (**Followed rename**), so that Takku's rename after Processing does not close my Meeting note.
79. As the user, I want a Note replaced by an atomic write from another app treated as changed, not deleted, so that Takku's and other editors' saves never look like deletions.
80. As the user, I want a Note deleted by another app to stay open with its text and a banner offering to save it again, so that I never lose what is on screen.
81. As the user, I want Obsidian and Noteki open on the same Vault at once to be safe, so that I can switch between them freely.

### Takku

82. As a Takku user, I want Noteki listed among Takku's Editors, so that Takku opens my Meeting notes in Noteki.
83. As a Takku user, I want Takku's *Show all* to show the Vault in Noteki, so that I can browse all my Meeting notes.
84. As a Takku user, I want to keep writing my Personal notes while Takku writes the Summary, so that the end of a Meeting is not a moment I must stop typing.
85. As the user, I want a Markdown file outside the Vault to open in its own window without being indexed, so that Noteki also works as a plain Markdown editor.
86. As the user, I want opening a folder other than the Vault to ask whether to switch Vault, so that nothing changes behind my back.

### Lightness

87. As the user, I want Noteki under 20 MB on disk, so that it is a fraction of Obsidian.
88. As the user, I want Noteki to show my last Note in under a second from a cold start, so that opening a Note is instant.
89. As the user, I want Noteki under 180 MB of memory at rest with one Note open, so that it can stay open all day.
90. As the user, I want memory not to grow with every Tab I open, so that keeping many Notes open costs nothing.

## Implementation Decisions

- **Three parts.** A Swift package for the domain (`NotekiCore`, no AppKit), the SwiftUI + AppKit app, and the editor in TypeScript on CodeMirror 6, loaded in a WKWebView ([ADR 0001](../../docs/adr/0001-codemirror-in-wkwebview.md)).
- **Everything about the Vault is in `NotekiCore`**: scanning and Excluded folders, Frontmatter parsing, Tags, Wikilink resolution, the index, search, the Quick switcher's matching, and the lifecycle of an open Note (load, edit, save, External change, Merge, Conflict, Followed rename). The editor receives data and reports edits; it decides nothing about the Vault.
- **One deep Vault module as the public face of `NotekiCore`.** It is opened on a folder and answers every query the app needs: list the tree, resolve a Wikilink from a given Note, Backlinks, Tags with counts and their Notes, search with `tag:` and `path:`, Quick switcher candidates, Outline. It opens Notes as **open-note** handles that take the user's edits and produce what to save and what to show.
- **The Vault is told about changes, it does not watch.** It exposes a single entry such as "these paths changed on disk". The app's FSEvents watcher calls it; tests call it directly. The Vault itself tells a rename (same file identity, new path) from a replacement (new identity, same path) from a deletion, by checking the file system when told.
- **Open-note lifecycle** ([ADR 0005](../../docs/adr/0005-merge-external-changes.md)): each open Note keeps the base text (the last text saved or loaded). Edits are saved about one second after the last change and on close. On an External change: reload if nothing is unsaved; three-way Merge on the base if something is; on a Conflict keep the user's text, stop saving, and report the Conflict with both versions so the app can show *Keep mine*, *Take theirs* and the differences.
- **Saving** writes the whole file atomically, keeping line endings and the final newline.
- **Index in memory** ([ADR 0006](../../docs/adr/0006-in-memory-index.md)), built at launch by reading every Note and updated incrementally from the change entry. Nothing is written to disk. The window shows the last Note before indexing ends.
- **Wikilink resolution follows Obsidian** ([ADR 0002](../../docs/adr/0002-coexist-with-obsidian.md)): a name with `/` is a path from the root; otherwise a file name anywhere. The rule for duplicate names is established by checking Obsidian by hand on the fixture Vault before the index is built, and recorded as tests. Unresolved links create the Note in the linking Note's folder.
- **Tags** are read from the `tags` Property (list or single value, with or without `#`) and from `#tag` in text outside code; a Tag starts with a letter and may hold letters, digits, `_`, `-`, `/`. Noteki writes Tags only in the `tags` Property, as a YAML list.
- **Frontmatter** that does not parse is never rewritten.
- **One web view per window, Noteki's own Tabs** ([ADR 0003](../../docs/adr/0003-one-web-view-per-window.md)): switching Tab swaps the CodeMirror editor state, keeping cursor, scroll and undo per Tab.
- **Live preview** is a set of CodeMirror extensions: inline syntax hidden outside the element under the cursor; blocks (tables, code, Frontmatter, callouts) rendered outside and source inside; image widgets; clickable checkboxes; Comments hidden. Tables are drawn in place, row by row (ADR 0008). Hiding syntax must never change line heights above the cursor. The Source mode toggle turns the extensions off.
- **Borrowed code** ([ADR 0007](../../docs/adr/0007-from-scratch-borrowing-mit-code.md)): web view handling (find, select-all, word movement, key handling) adapted from MarkEdit; Live preview extensions adapted from SilverBullet and MarkEdit-preview; all MIT and listed in a third-party file. Nothing from Zettlr (GPL) or Obsidian.
- **Bridge** between app and editor: a typed message enum on each side, written by hand, about ten messages: load a Note with its configuration, report an edit, swap the Tab state, follow a link, ask for completions (Notes and Tags), resolve Wikilinks and Embeds for display, toggle a checkbox, find, select-all, toggle Source mode.
- **Editor bundle** built into one HTML file with everything inlined and committed; the app injects configuration before loading it; a build phase fails when the bundle does not match a hash of its sources ([ADR 0004](../../docs/adr/0004-committed-editor-bundle.md)).
- **Takku integration** needs no change to Takku: the app declares the Editor role for Markdown and plain text and declares folders, so Takku lists it and hands it the Vault for *Show all*. Takku's atomic writes and its rename after Processing are covered by the open-note lifecycle.
- **Not sandboxed**, signed with the user's Developer ID, macOS 15+, Apple Silicon.
- **Milestones**, each usable daily: M1 read and write (Vault, Files pane, Source mode editor, saving, External changes with Merge, Conflict and Followed rename, images and PDFs, Takku document types); M2 the Vault (index, Wikilinks, Quick switcher, Backlinks, Tags, Search, Outline); M3 Live preview (hidden syntax, tables, images, tasks, callouts, Comments, Properties with Tag chips); M4 the rest (Tabs, Pasted and dropped images, Daily note, file operations, settings).

## Testing Decisions

- **A good test drives the module from outside and checks what a user would see**: given these files on disk and these events, this is what the Vault answers or what an open Note shows and saves. Tests never reach into the index's structures, the Merge algorithm's steps or CodeMirror's internals, so any of them can be rewritten without touching a test.
- **Seam 1, the Vault module of `NotekiCore` (the main one).** Each test builds a Vault in a temporary folder, writes, renames, replaces and deletes real files, calls the change entry, and asserts only through the Vault's public queries and open-note handles. It covers scanning and Excluded folders, Frontmatter, Tags, Wikilink resolution (duplicates, aliases, paths, Embeds, Unresolved links), Backlinks, search, the Quick switcher, Outline, saving, Merge, Conflict, Followed rename, replacement and deletion. Real FSEvents are not used.
- **Takku's writes are replayed** at Seam 1: the sequence of atomic writes and the rename that Takku performs during Processing, interleaved with a user still typing in the same Note, must lose nothing.
- **A fixture Vault** in the repository, written for the purpose (duplicate names, aliases, Embeds, nested Tags, broken YAML, Excluded folders, a repository inside the Vault). Never the user's real Vault.
- **Seam 2, the CodeMirror editor state with the Live preview extensions.** Given a text and a cursor position, the test checks what is hidden, how a table's rows are laid out, what is a widget and what a click on a checkbox writes. It runs without a browser, on the editor state alone.
- **A contract test on the bridge** checks that the message names match on both sides.
- **No automated tests on the SwiftUI/AppKit layer**: it is kept thin and checked by hand.
- **A measurement script**, run at the end of every milestone, generates a Vault of 5,000 Notes and measures the four budget numbers: size on disk, cold start, indexing time, memory at rest.
- **Prior art**: TakkuCore's Swift Testing suites in the sibling Takku project (Meeting note, Speaker names and Transcript tests), which drive Markdown and Frontmatter through public types on fixture text.

## Out of Scope

- Renaming a Note updates the Wikilinks that point to it: right after v1.
- Rendering Mermaid and math; links to Headings and blocks; embedding a Note in another; unlinked mentions in Backlinks.
- More Vaults at once; a command palette; a persistent index.
- Graph view, canvas, plugins, themes and CSS snippets, sync, git, publishing, an iOS app.
- Reading Obsidian's own settings (excluded files, attachment folder, Daily note folder).
- Any change to Takku.

## Further Notes

- The user's Vault today: 624 Notes; Wikilinks in 236, the `tags` Property in 123, tables in 108, code blocks in 73, Comments in 64 (Takku's markers), task lists in 32, aliases in 27, Embeds in 11 (all Pasted images), callouts in 3, Mermaid in 2; no Heading links, math, footnotes or Dataview. About 13,000 SVG files and several code repositories live under two folders that the Excluded folder rules leave out.
- Known risks, in the order they bite: the WebKit process alone may use most of the memory budget (measure at M1); Obsidian's rule for duplicate names is undocumented (check before M2); native text behaviour in the web view must be wired by hand; Live preview may make the text jump (try on the longest Meeting notes); frequent Conflicts if Takku rewrites most of a Note at once (then take the base when the user starts typing).
- The project folder is not a git repository yet.
