# 21: Image Embeds

**What to build:** `![[image.png]]` and `![](image.png)` show the image in the Note, at most as wide as the text. `![[file.pdf]]` is a link that opens the PDF.

**Blocked by:** 19 (Live preview of inline syntax), 03 (Images and PDFs in native views)

**Status:** ready-for-agent

- [x] Image Embeds render inline, scaled to the text width
- [x] The Embed's source appears when the cursor is on it
- [x] A PDF Embed opens the PDF in its native view
- [x] Missing images show their name as an Unresolved link
- [x] Tested at Seam 2

## Comments

Done.
- `![[image.png]]` and `![](image.png)` draw the image under their line, as wide as the text at most (or `|300` pixels).
- The app finds the file as a Wikilink is found, and reads the image's size from its file, away from the main thread. The editor reserves the room before the image loads.
- The image is always drawn; only the Embed's source hides while the cursor is away. With the cursor on the line the source shows and the image stays, so nothing moves. An empty line takes the source's place: it is the price of that.
- A PDF Embed shows its name or Alias as a link that opens the PDF natively. A missing image's Embed shows its name, faded as an Unresolved link.

Images reach the web view through a `noteki-file:` scheme that serves only images inside the Vault, outside Excluded folders, symlinks resolved. Images larger than 1,600 pixels are served made smaller (a JPEG, or a PNG with transparency): WebKit keeps images decoded, and the 6,016-pixel HEIC in the demo Vault would have taken about 100 MB. An image changed on disk is loaded again: its version is in the address.

Tests: Seam 1 (resolution, sizes, PDFs, Notes, web addresses, serving only from the Vault, smaller images, versions) and Seam 2 (what hides, where images go, decoded URLs, PDF Aliases, missing images, tables).

Not done, in the SPEC: an image in a table stays as written; in a quote or a list the image starts at the margin; a Loose file's window embeds nothing; a Markdown image's path is read as a Wikilink's, so a `#` or `|` in a file name would cut it.

Fixed after review:
- image headers were read on the main thread;
- an image changed on disk stayed as it was;
- large images were decoded at full size;
- the estimated height ignored the drawn width;
- a click on an image's hidden source opened it instead of placing the cursor;
- a click on a missing Markdown image's text created a Note;
- a PDF's Alias showed `Doc.pdf|alias`;
- an image in a table broke its rows;
- Excluded folders were served;
- a Loose file's folder was served.

Verified in the app: a 200-pixel image with `|200`, a 6,016-pixel HEIC scaled to the text width, the PDF as "Contract.pdf", the missing image faded, and with the cursor on the Embed's line the source shown with the image still under it. The click on the PDF and the fixes after review were not tried in the app: Noteki could not be brought forward without taking the keyboard from the user, and once it did take some of the user's typing (restored).
