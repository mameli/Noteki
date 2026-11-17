# 13: Settings

**What to build:** A Settings window (⌘,) with what SPEC › Window lists and the prototype lacks: font family (any installed), code font, line length (40–200 characters or no limit), Excluded folders of the Vault (add, remove), Daily note folder and date format; text size and theme, already there, shown too. Applied at once.

**Blocked by:** none

**Status:** done

- [x] Each setting saved, loaded, applied to the editor or the Vault scan
- [x] Tests: settings round trip; UI (changing the line length rewraps)

## Comments

- ⌘, (and "Settings…" in the Command palette) opens one Settings window, built on GPUI Kit's `setting` component: pages Appearance (theme, light/dark), Text (font, code font, size, line-length limit and length), Excluded folders (list, Remove, Add folder… with `core::vault::folder_to_exclude`), Daily note (folder, format, today's path shown).
- Every change goes through `NotekiApp::update_settings`, deferred to the Vault's window: saved, then applied at once (theme, editor fonts and width, Vault rescan). A folder newly left out closes its Tabs, saved first.
- The line width is `length × size / 2` (half an em per character); no limit fills the window.
- Font lists offer every installed family for both fonts (no fixed-width filter: YAGNI). A Loose file's window takes the text settings when it opens, not later changes.
- Not checked on screen (it would take the keyboard): the window is covered by a UI test that renders it and picks a theme with real clicks.
