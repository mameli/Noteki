# 15: Backlinks

**What to build:** The right sidebar lists the Notes with a Wikilink to the Note in front, each with the line that holds the link. Clicking one opens that Note at that line.

**Blocked by:** 11 (Vault index and clickable Wikilinks)

**Status:** ready-for-agent

- [x] Collapsible right sidebar with a Backlinks section
- [x] Each Backlink shows its Note's name and the line around the link
- [x] Clicking opens the Note at the link
- [x] Aliases and path links count; unlinked mentions do not
- [x] Tested at Seam 1

## Comments

Done. The right sidebar (⌥⌘0, the toolbar button, or the View menu) lists the Notes that link to the Note in front, each with its name, its folder and the line that holds the link, cut around the link when it is long. Clicking one opens that Note with the cursor on that line; `load` carries the line to the editor.

Seam 1 tests cover: the line, Aliases, headings, blocks and paths, unlinked mentions (not listed), links in code and fences, the editor's rules for what is a link, escaped brackets and table pipes, links to the Note itself, same-named Notes resolved as a click would, order, line counting with `\n`, `\r\n` and `\r`, long lines, Frontmatter, rescanning and removing, and a link written before its Note exists.

Decisions to know:
- **The sidebar is hidden at first, and the links are read when it is first shown.** Reading them at launch put the app over the memory budget: 154–161 MB against 150. With the sidebar hidden it is 149 MB (146 before this ticket, same method), with it shown 155. The sidebar itself costs about 10 MB, the links about 4 MB. If you leave it open, you are 5 MB over. The SPEC says this.
- **The links are kept as 8 bytes each**, the file name they ask for and the line. The text of the lines is read from the files when Backlinks are asked for. ADR 0006 now says Backlinks do not keep text in memory, and that Tags and search have the same choice.
- Embeds `![[Note]]` and links in the Frontmatter count, as in Obsidian; CONTEXT says so. Heading and block links count too, since they lead to the Note.
- A link typed in a Note shows in the Backlinks of its target after the Note is saved, a moment later.
- Not excluded: indented code blocks, fences nested in a list, a code span across lines, `%%comments%%`.

Reading 5,000 Notes with 45 links each: 1.5 s. The first time after the files were generated it took 16 s, because macOS looks at new files; it is not the scan.

Found while testing and fixed: names given the same id (the map found nothing for a Note), a SwiftUI List costing several MB, and the title of the window taken over by the sidebar.

Verified in the app by pressing the sidebar and Backlink buttons, without taking the keyboard: Backlinks listed, updated when a file changed on disk, and a click opened the Note with the cursor on the right line. A scroll to line 150 of 300 was checked on the editor page.
