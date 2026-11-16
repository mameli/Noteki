# 04: Text size with ⌘+, ⌘− and ⌘0

**What to build:** ⌘+ (and ⌘=) makes the text one point larger, ⌘− one smaller, between 10 and 32 points; ⌘0 puts 15 back. The size is a setting, remembered. Headings, code and tables scale with it (they are relative to it already).

**Blocked by:** none

**Status:** ready-for-agent

- [ ] `font_size` in the settings (default 15), saved and loaded
- [ ] The editor shapes with it; changing it reshapes every line once (the shape cache's look changes)
- [ ] Tests: settings round trip and limits; UI (⌘+ makes a line taller, ⌘0 back)
