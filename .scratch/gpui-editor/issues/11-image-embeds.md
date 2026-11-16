# 11: Images in the Note

**What to build:** A line holding only an image Embed `![[image.png]]` (or `![alt](path)`) is drawn as the image, as wide as the text at most, keeping its proportions, while the cursor is off the line; on it, the line shows as written with the image under it. Paths are found as SPEC › Embeds says (the Vault index already resolves them). This brings in **block lines**: a line drawn as something of its own height.

**Blocked by:** 07

**Status:** ready-for-agent

- [ ] The image's size is read without decoding it (`imagesize`, already a dependency) so the line's height is known before the image loads; the image loads in the background and is drawn when ready
- [ ] With the cursor on the line the source shows above the image, and the image stays: the lines above never move
- [ ] A missing image draws as its source, in the unresolved-link colour
- [ ] A click on the image puts the cursor on its line
- [ ] "Immagini" of `rendering-test.md` shows `noteki.png` (in `docs/demo-vault/Media`)
- [ ] Tests: the block height from the image size and the width; UI: the cursor going down through the image's line never moves the lines above
