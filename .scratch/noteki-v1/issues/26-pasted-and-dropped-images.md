# 26: Pasted and dropped images

**What to build:** Pasting an image saves it as `Pasted image YYYYMMDDHHmmss.png` in the Note's folder and writes its Embed at the cursor. Dropping an image file from the Finder copies it next to the Note, keeping its name, and writes its Embed.

**Blocked by:** 21 (Image Embeds)

**Status:** ready-for-agent

- [x] Pasting an image creates the file and the Embed
- [x] Dropping an image file copies it and writes the Embed
- [x] Names never overwrite an existing file
- [x] Tested at Seam 1 for file naming

## Comments

Done.
- Pasting an image saves it in the Note's folder as `Pasted image YYYYMMDDHHmmss.png`, PNG whatever was copied, and writes its Embed in place of the selection.
- Dropping image files from the Finder copies them next to the Note under their names and writes their Embeds where they were dropped, one per line. Image files copied in the Finder paste the same way.
- A name already taken, in any letter case, gets ` 1`, ` 2`…; no file is overwritten, even one written meanwhile by another app.
- An image the Vault already lists is not copied: its Embed is its name, or its path when the name leads elsewhere from that Note. One in an Excluded folder is copied.
- What a Wikilink cannot hold (`[ ] | # ^`, and `\ :`) becomes spaces in a copy's name.
- Text on the pasteboard wins over an image (a copy from Word or Numbers carries a picture of itself).
- Undo takes the Embeds out; the files stay.

Paste and drop are taken in the native web view, not in the editor's script: the app reads the pasteboard and the files' paths itself, and no image crosses the bridge. WKWebView's own `paste:` is called through the runtime when there is no image for the app. The editor gets an `insert` command with the text, and the drop's point in CSS pixels.

Tests: Seam 1 (names, collisions in any case, several files, files already in the Vault, Excluded folders, characters left out, a file that cannot be copied) and Seam 2 (where the text goes, the selection replaced, one undo).

Not done, in the SPEC: an image dragged from a browser (data, no file) is not taken; Obsidian's attachment folder setting is not read; files other than images are left out of a drop. A large file dropped from another disk is copied on the main thread (on the same disk APFS clones it at once).

Fixed after review:
- a drop that failed partway left copies without Embeds: the images copied are embedded, the others left out;
- a drop the app would not take was lost instead of going to WebKit, and showed the copy cursor;
- the Edit menu decoded the pasted image on every validation: it now reads the types only;
- files were written even when the editor could not write their Embeds.

Verified in the app, on the demo Vault: a PNG pasted with ⌘V after "Notes:" (file, Embed, image drawn, listed in Files at once); Whiteboard.png and notes.txt dragged from a Finder window (the image copied and embedded exactly at the drop point, the text file left out); `[draft] plan #2.png` and `Media/Wallpaper.HEIC` copied in the Finder and pasted (`draft plan 2.png` copied, the HEIC embedded by name and not copied); ⌘Z took the Embeds out and left the file; plain text pasted as text; Edit → Paste from the menu with an image on the pasteboard. The clipboard was saved before and put back after.
