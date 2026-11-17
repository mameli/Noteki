# 10: Pasted and dropped images

**What to build:** An image pasted (⌘V with an image on the clipboard) is saved as `Pasted image YYYYMMDDHHMMSS.png` next to the Note and an Embed `![[...]]` is put at the cursor, as Obsidian does; an image file dropped on the editor from the Finder is copied next to the Note under its own name (a number added if taken) and embedded where it was dropped. SPEC › Embeds and Attachments, **Pasted image**.

**Blocked by:** none

**Status:** ready-for-agent

- [x] GPUI's `ClipboardEntry::Image` and `ExternalPaths` drops
- [x] The name never overwrites a file; the Vault index learns the new Attachment
- [x] Tests: naming without GPUI; UI (an image on the test clipboard pasted gives a file and an Embed)

## Comments

Done (2026-11-17). `core::attachment`: Obsidian's `Pasted image YYYYMMDDHHMMSS.png` name, a free path (`name 1.png`, `name 2.png`... never overwriting), which dropped files are images. ⌘V with an image and no text on the clipboard (a screenshot) emits `PasteImage`; files dropped on the text move the cursor where they were dropped and emit `DropFiles`. The app writes or copies them next to the Note in front, tells the Vault (so the Embed resolves at once) and embeds `![[name]]` at the cursor, one per line for several. Other dropped files are left alone. Tests: names and free paths; UI (a PNG on the test clipboard pasted gives a file next to the Note and its Embed; a real file-drop event copies the file and embeds it where dropped). New mutant: overwriting. Killed.
