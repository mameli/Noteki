# 02: Five window-sized surfaces kept after a burst of frames

**What to build:** after a burst of frames (opening a Vault of 665 Notes, and before 0.4.1 an image loading) macOS keeps five surfaces as large as the window (16.3 MB each at 1280×805 on a Retina screen, 80 MB) instead of two, for as long as the window is open. On the user's own Vault at rest Noteki is 127 MB, 80 of them these surfaces. GPUI's renderer sets `maximumDrawableCount(3)` on its `CAMetalLayer` (gpui-pre-apple, `metal_renderer.rs`), and on macOS 14+ drives frames with a `CAMetalDisplayLink` (gpui-pre-macos, `window.rs`), which appears to hold drawables of its own. Find which keeps the extra ones and bring them back to two at rest: a `[patch]` of the GPUI crate (`maximumDrawableCount(2)`, or releasing the display link's drawables when it is detached at idle), or a change asked upstream.

**Why:** the largest part of Noteki's memory at rest on a real Vault (127 MB, Obsidian with two windows 749 MB): two surfaces would make it about 80 MB.

**Blocked by:** none

**Status:** needs-triage

- [ ] Reproduced headlessly enough to count: `vmmap <pid> | grep IOSurface` shows five 16.3M regions after opening a large Vault, two after opening the demo vault
- [ ] The surfaces back to two at rest, without frames dropped while typing or scrolling (`keystroke_timing`, and by hand on a 120 Hz screen)
- [ ] Measured as in `.scratch/performance/spec.md`, before and after, in the commit message

## Comments
