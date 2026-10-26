# Noteki

A light native macOS app to read and write a folder of Markdown notes, the same folder Obsidian opens. It was born to read and edit the Meeting notes that Takku writes, without the weight of Obsidian.

## Language

### Vault and files

**Vault** (`Vault`):
The folder of Notes and Attachments that Noteki has open. One at a time. It can be an Obsidian vault, and stays one.
_Avoid_: workspace, library, folder (for the whole)

**Note** (`Note`):
A Markdown file in the Vault, the thing the user reads and writes.
_Avoid_: document, page, file (when the Markdown matters)

**Attachment** (`Attachment`):
An image or a PDF in the Vault. Noteki shows it but does not edit it.
_Avoid_: asset, media, resource

**Pasted image** (`PastedImage`):
An image the user pastes into a Note: Noteki saves it as an Attachment next to the Note and puts an Embed of it where the cursor is. An image file dropped on a Note from the Finder is copied next to it the same way, under its own name, but is no Pasted image.
_Avoid_: screenshot (that is Takku's), clipboard image

**Excluded folder** (`ExcludedFolder`):
A folder inside the Vault that Noteki neither lists nor indexes: hidden folders, environments and dependencies of code, any folder that is a repository of its own, and the ones the user adds.
_Avoid_: ignored path, filter

**Loose file** (`AppModel.looseFile`):
A Markdown or text file from outside the Vault, opened in a window of its own: written and saved like a Note, with nothing of the Vault around it.
_Avoid_: standalone file, external note

**Daily note** (`DailyNote`):
The Note of a given day, named after its date in a folder the user chooses; the date's format may make folders too. Noteki opens it, creating it if missing.
_Avoid_: journal, diary

### Inside a Note

**Frontmatter** (`Frontmatter`):
The YAML block at the top of a Note that holds its Properties.
_Avoid_: header, metadata block

**Property** (`Property`):
One key and its value in the Frontmatter, such as `participants` or `duration`.
_Avoid_: field, attribute, metadata

**Tag** (`Tag`):
A label of a Note, written in the `tags` Property or as `#tag` in the text. A `/` makes a Tag nested (`client/axo`), and the Tags form a tree. `#Client` and `#client` are one Tag, shown as most Notes write it.
_Avoid_: label, category, keyword

**Comment** (`Comment`):
Text between `%%` marks: part of the Note, hidden while reading. Takku uses it to mark the part of the Meeting note it rewrites.
_Avoid_: hidden text, annotation

**Task** (`Task`):
An item of a list that starts with `[ ]`, or `[x]` once done: shown with a checkbox the user ticks.
_Avoid_: todo, checkbox (that is how it is drawn)

**Callout** (`Callout`):
A quote whose first line starts with `[!type]`, such as `> [!warning] Title`: drawn as a box in the colour of its type, with its title.
_Avoid_: admonition, alert, box

**Heading** (`Heading`):
A Markdown title in a Note: a line starting with `#` to `######`, or a paragraph underlined with `===` or `---`. The Headings of a Note make its Outline.
_Avoid_: section, title (that is the Note's name)

### Links

**Wikilink** (`Wikilink`):
A link to a Note written `[[name]]`, by name and not by path, as Obsidian does.
_Avoid_: internal link, reference

**Alias** (`Wikilink.alias`):
The text a Wikilink shows instead of the Note's name: `[[name|alias]]`.
_Avoid_: label, display name

**Embed** (`Embed`):
A Wikilink with `!` in front, `![[file]]`: an image is shown in the Note, a PDF becomes a link to it. A Markdown image `![](file)` is shown the same way, its path found as an Embed's is.
_Avoid_: transclusion, inline image

**Unresolved link** (`UnresolvedLink`):
A Wikilink whose name matches no Note in the Vault, or an Embed of a file that is not there. Following a Wikilink to a missing Note creates that Note.
_Avoid_: broken link, dead link

**Backlink** (`Backlink`):
A Wikilink in another Note that points to the Note being looked at. An Embed of the Note and a link in the Frontmatter count too, as in Obsidian.
_Avoid_: incoming link, reference, mention

### Reading and writing

**Live preview** (`LivePreview`):
The way Noteki shows a Note: the Markdown is formatted in place, and its syntax appears only where the cursor is, so the same view is read and written.
_Avoid_: WYSIWYG, rich text, reading mode

**Source mode** (`SourceMode`):
The other way to show a Note: all the Markdown visible as written, nothing hidden.
_Avoid_: raw mode, plain text, code view

**Tab** (`Tab`):
A Note or an Attachment open in a window. A window has many Tabs and shows one; each Tab keeps its own cursor, scroll and undo, and remembers what it showed before, for back and forward.
_Avoid_: pane, document window

**Search** (`SearchQuery`):
Finding the Notes of the Vault that hold every word typed, narrowed by `tag:` and `path:`. Not **Find**, which looks inside the Note in front.
_Avoid_: find in files, full-text search

**Quick switcher** (`QuickSwitcher`):
The box that opens a Note by typing part of its name.
_Avoid_: open dialog, finder, palette

**Outline** (`Outline`):
The list of the Headings of the Note in front, to jump to one.
_Avoid_: table of contents, TOC

### Changes from outside

**External change** (`ExternalChange`):
A Note changed on disk by something other than Noteki, such as Takku writing its Summary or Obsidian open on the same Vault.
_Avoid_: remote change, sync, update

**Merge** (`Merge`):
Putting an External change together with what the user wrote and Noteki has not saved yet, starting from the last version both had in common.
_Avoid_: reconcile, sync

**Conflict** (`Conflict`):
An External change and the user's edits that touch the same lines, or adjacent ones, so the Merge cannot put them together. The user's text is kept and the user decides.
_Avoid_: clash, collision

**Deleted Note** (`OpenNote.isDeleted`):
An open Note whose file another app deleted or moved out of the Vault: its text is only on screen, nothing is saved until the user saves it again.
_Avoid_: orphan, missing note

**Followed rename** (`FollowedRename`):
A Note renamed or moved by something other than Noteki while open: its Tab follows it to the new name instead of saying it was deleted.
_Avoid_: tracking, relink
