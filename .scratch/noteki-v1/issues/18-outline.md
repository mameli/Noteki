# 18: Outline

**What to build:** The right sidebar shows the Headings of the Note in front as an indented list; clicking one moves the editor to it.

**Blocked by:** 04 (Write and save a Note)

**Status:** ready-for-agent

- [x] Headings listed with their level as indentation, updated while typing
- [x] Clicking a Heading scrolls the editor to it
- [x] Headings inside code blocks are ignored

## Comments

Done. The right sidebar has a control at its top, Outline or Backlinks, remembered in the preferences; the Outline comes first and is the default. It lists the Headings of the Note in front, indented by level, from the text as the editor reports it, so it follows typing. A click puts the cursor at the start of the Heading's line, in the middle of the view, and gives the editor the keyboard.

`Heading.all(in:)` (and `OpenNote.outline`) in NotekiCore, tested at Seam 1: `#` to `######` with closing `#`s and indentation, an empty Heading left out, setext Headings at their first line, a rule under a list item, a quote or another rule not a Heading, fenced code and the Frontmatter skipped, line endings, and the text shown without the marks of links, emphasis and code.

Changed with it: the links of the Vault are now read when the Backlinks pane is first shown, not when the sidebar is: with the sidebar on its Outline they cost nothing.

Not handled yet, in the SPEC: Headings inside a quote, a list item or a `%%` Comment.

Fixed after review: a setext Heading of several lines jumped to its last line, `- item` or `> quote` above `---` became a Heading, the Heading text showed `[[ ]]` and `**`, a Loose file's window computed an Outline it has no sidebar for.

Verified in the app by typing: the Headings listed and indented, a Heading typed appearing, a `#` line in a fenced block not listed, and clicks landing on lines 5 and 13.

Memory: the right sidebar on its Outline costs about 10 MB of Noteki's own process, the Backlinks pane about 6 MB more (CHANGELOG).
