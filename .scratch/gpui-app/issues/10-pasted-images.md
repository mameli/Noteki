# 10: Pasted and dropped images

**What to build:** An image pasted (⌘V with an image on the clipboard) is saved as `Pasted image YYYYMMDDHHMMSS.png` next to the Note and an Embed `![[...]]` is put at the cursor, as Obsidian does; an image file dropped on the editor from the Finder is copied next to the Note under its own name (a number added if taken) and embedded where it was dropped. SPEC › Embeds and Attachments, **Pasted image**.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] GPUI's `ClipboardEntry::Image` and `ExternalPaths` drops
- [ ] The name never overwrites a file; the Vault index learns the new Attachment
- [ ] Tests: naming without GPUI; UI (an image on the test clipboard pasted gives a file and an Embed)
