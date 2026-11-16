# 11: Images in the Note

**What to build:** A line holding only an image Embed `![[image.png]]` (or `![alt](path)`) is drawn as the image, as wide as the text at most, keeping its proportions, while the cursor is off the line; on it, the line shows as written with the image under it. Paths are found as SPEC › Embeds says (the Vault index already resolves them). This brings in **block lines**: a line drawn as something of its own height.

**Blocked by:** 07

**Status:** ready-for-agent

- [x] The image's size is read without decoding it (`imagesize`, already a dependency) so the line's height is known before the image loads; the image loads in the background and is drawn when ready
- [x] With the cursor on the line the source shows above the image, and the image stays: the lines above never move
- [x] A missing image draws as its source, in the unresolved-link colour
- [x] A click on the image puts the cursor on its line
- [x] "Immagini" of `rendering-test.md` shows `noteki.png` (in `docs/demo-vault/Media`)
- [x] Tests: the block height from the image size and the width; UI: the cursor going down through the image's line never moves the lines above

## Comments

Done (2026-11-16).

- `core::markdown::image_embed`: a line holding only `![[target]]` or `![alt](path)`. `style_lines` takes a `Context` with two closures the editor gives: code colours (ticket 09) and the image a target names. An image line gets `Decoration::Image` (absolute path, width, height) and, off the cursor, its whole source hidden; a missing image stays as its source.
- The editor resolves a target with the existing `resolve_embed` (Obsidian's resolution, `imagesize` reading only the file's header) and keeps the result per target until the Vault index changes. `set_link_context` now takes a `LinkContext` (the Note's path, the index, the Vault's folder).
- The element sizes the line from the image's size, as wide as the file or the text if narrower, in proportion, so its height is known before the image is decoded. The image is loaded with GPUI's `use_asset::<ImageAssetLoader>` (in the background, cached, the view redrawn when ready), a placeholder box until then, and only when its line is on screen.
- With the cursor on the line, the source shows as a row above the image (that line grows by one row; the lines above never move). A click on the image puts the cursor on its line (at the end of its source).
- Tests: `image_embed`; the image decoration and hidden source; UI on the demo Vault's `noteki.png` (the image's height, a missing image as one line, the cursor showing the source above the image without moving the line above). New mutant: the image dropped on the cursor's line. Killed. Timing unchanged.
