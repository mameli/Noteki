# 02: Five window-sized surfaces kept after a burst of frames

**What to build:** after a burst of frames (opening a Vault of 665 Notes, and before 0.4.1 an image loading) macOS keeps five surfaces as large as the window (16.3 MB each at 1280×805 on a Retina screen, 80 MB) instead of two, for as long as the window is open. On the user's own Vault at rest Noteki is 127 MB, 80 of them these surfaces. GPUI's renderer sets `maximumDrawableCount(3)` on its `CAMetalLayer` (gpui-pre-apple, `metal_renderer.rs`), and on macOS 14+ drives frames with a `CAMetalDisplayLink` (gpui-pre-macos, `window.rs`), which appears to hold drawables of its own. Find which keeps the extra ones and bring them back to two at rest: a `[patch]` of the GPUI crate (`maximumDrawableCount(2)`, or releasing the display link's drawables when it is detached at idle), or a change asked upstream.

**Why:** the largest part of Noteki's memory at rest on a real Vault (127 MB, Obsidian with two windows 749 MB): two surfaces would make it about 80 MB.

**Blocked by:** none

**Status:** needs-info (the fix is a choice between a patched GPUI and the memory)

- [ ] Reproduced headlessly enough to count: `vmmap <pid> | grep IOSurface` shows five 16.3M regions after opening a large Vault, two after opening the demo vault
- [ ] The surfaces back to two at rest, without frames dropped while typing or scrolling (`keystroke_timing`, and by hand on a 120 Hz screen)
- [ ] Measured as in `.scratch/performance/spec.md`, before and after, in the commit message

## Comments

### 19 November 2026: what keeps them

- **The burst.** On the user's Vault, opening a Note deep in a folder reveals it in the tree. The tree scrolls, and its scrollbar fades in (GPUI Kit's `SCROLLBAR_ENTER`, 300 ms) and later out (`SCROLLBAR_EXIT`, 500 ms). That is two runs of frames at 120 Hz, 24 and about 60 frames. With the reveal turned off there is no burst, 3 drawables and 77 MB, against 5 drawables and 129 MB. Any scroll (tree, editor, Outline) fades a scrollbar the same way, so a Vault in use ends at 5 even without the reveal.
- **Where they are.** `vmmap` names all five `'CAMetalLayer Display Drawable'`, 2560×1642 (the default window on Retina). Logging the drawable each frame: 2 distinct before the burst, 5 after.
- **What does not release them**, each tried with a patched gpui-pre-apple / gpui-pre-macos:
  - `setMaximumDrawableCount(2)` at idle: Core Animation raises `CAMetalLayerInvalidOperation: -setMaximumDrawableCount should not be called when using CAMetalDisplayLink`. The exception is swallowed by the display link's callback, so the rest of GPUI's frame is skipped without a word. The `maximumDrawableCount(3)` in `metal_renderer.rs` has no effect while the link drives the layer.
  - `preferredFrameLatency` 1 instead of 2, while paused or from the start: still 5.
  - Invalidating the link and making a new one after a burst: still 5. The pool belongs to the layer.
  - Detaching the link, then setting `maximumDrawableCount` 2 and back to 3 (allowed then), then attaching again: still 5.
- **What does.** Without `CAMetalDisplayLink` (GPUI's `CVDisplayLink` path, the one used before macOS 14), the layer keeps its limit: 3 drawables, 95 MB after the same burst. In gpui-pre-macos it is a single condition in `window.rs` (`MetalFrameSource::is_available()`). It costs a patched copy of the crate (548 KB) kept in step with each GPUI update. It also brings back `nextDrawable`, which can wait on the compositor when the GPU is behind: this is what the display link was adopted to avoid. Typing and scrolling on a 120 Hz screen would have to be checked by hand.
- **Not possible:** two at rest with the display link. Core Animation sizes that pool itself.

