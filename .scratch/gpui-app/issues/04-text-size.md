# 04: Text size with ⌘+, ⌘− and ⌘0

**What to build:** ⌘+ (and ⌘=) makes the text one point larger, ⌘− one smaller, between 10 and 32 points; ⌘0 puts 15 back. The size is a setting, remembered. Headings, code and tables scale with it (they are relative to it already).

**Blocked by:** none

**Status:** ready-for-agent

- [x] `font_size` in the settings (default 15), saved and loaded
- [x] The editor shapes with it; changing it reshapes every line once (the shape cache's look changes)
- [x] Tests: settings round trip and limits; UI (⌘+ makes a line taller, ⌘0 back)

## Comments

Done (2026-11-17). `Settings::font_size` (15 by default, a settings file from before reads as 15), `zoomed` keeps it within 10–32. ⌘= / ⌘+ / ⇧⌘= larger, ⌘− smaller, ⌘0 back to 15, saved at once. The editor shapes with it instead of the window's text style; Headings, code and tables scale since they are relative to it; the shape cache reshapes every line once, since its look changed. Tests: limits and an old settings file; UI (two steps up make a line taller and are saved, one down, ⌘0 back to the first height). New mutant: the size not saved. Killed.
