# 03: Images and PDFs in native views

**What to build:** Images (`png`, `jpg`, `jpeg`, `gif`, `webp`, `svg`, `heic`) and PDFs appear in the Files pane as Attachments and open in the centre in a native view: an image view for images, PDFKit for PDFs. Every other file type stays hidden.

**Blocked by:** 01 (Open a Vault and read a Note)

**Status:** ready-for-agent

- [x] Attachments are listed in the Files pane next to Notes; other file types are not
- [x] Clicking an image shows it scaled to fit, at most at its own size
- [x] Clicking a PDF shows it in a PDFKit view with scrolling and zoom
- [x] Tested at Seam 1: which files the Vault lists as Notes and Attachments

## Comments

Done. Verified in the app on a demo Vault: PNG, SVG (sized and viewBox-only), a 9 MB HEIC scaled to fit, a PDF with scrolling, trackpad zoom and View → Zoom In/Out (⌘+ / ⌘−, disabled when no PDF is shown), and a message for an unreadable image or PDF. Extensions match in any case (`.JPG`); Notes still need a lowercase `.md`. Not done: images are not downsampled, a very large photo is decoded at full size.
