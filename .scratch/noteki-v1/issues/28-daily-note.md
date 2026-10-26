# 28: Daily note

**What to build:** *File → Today's Note* (⌥⌘D) opens today's Note, creating it if missing, in a folder and with a date format chosen in Settings (defaults `Journal/Daily_2025` and `YYYY-MM-DD`).

**Blocked by:** 04 (Write and save a Note)

**Status:** ready-for-agent

- [x] ⌥⌘D opens today's Note, creating it if missing
- [x] Folder and date format are configurable
- [x] Tested at Seam 1 for the path computed

## Comments

Done, with two changes to what was asked, both recorded in the SPEC:
- **The shortcut is ⌘⇧D**, not ⌥⌘D: macOS takes ⌥⌘D to hide the Dock, and the app never gets it (found in the app: it turned the user's Dock hiding off).
- **The default is a folder for each year**: folder `Journal`, format `[Daily_]YYYY/YYYY-MM-DD`, so 2025's Notes stay where they are (`Journal/Daily_2025/2025-09-16.md`) and today's goes in `Journal/Daily_2026/`. With `Journal/Daily_2025` alone, 2026's Notes would have gone in 2025's folder (the user's choice, 26 October 2026).

*File → Today's Note* opens the Daily note in the Tab in front, the Note open saved first, and the editor takes the keyboard. Missing, it is created empty (no template), its folders too, under their names on disk.

The format is Obsidian's, moment.js's tokens: years, quarters, months, days, weekdays, ISO and locale weeks, ordinals, times; not the localized `L`…. Names of days and months are English, whatever language Obsidian is set to. Text in `[brackets]` or after `\` stays as written; a `/` makes folders. Settings has a *Daily Note* pane with the folder, the format and today's path as it is typed. One setting for the app, where the Excluded folders are per Vault.

Refused with an alert: a Daily note in an Excluded folder, and a name that is empty, leaves its folder (`..`), or holds `\ :` or `[ ] # ^ |`.

`Vault.existingFile` now gives the on-disk letter case of the folders too, not only of the file: a Note created through `journal/daily_2025` is listed, and opened, as `Journal/Daily_2025/…`.

Tests: Seam 1 (the default path and a year later, tokens, ordinals, times, escapes, ISO and locale weeks at the new year, literals and folders, blanks, creating and finding the Note, folders in other letter case, Excluded folders, refused formats), on the Gregorian calendar whatever the Mac uses.

Fixed after review:
- a new Daily note in folders typed in other letter case never opened;
- tokens moment formats came out as written (times, am/pm, ordinals, `\` escapes);
- a format could leave its folder (`[..]`) or make an empty name;
- the opening skipped saving the Note open, and its alert blamed the format for every error;
- the tests depended on the Mac's calendar.

Verified in the app, on the demo Vault: ⌘⇧D created `Journal/Daily_2026/2026-10-26.md` and its folders, opened it with the editor taking the keyboard ("today" typed there); ⌘⇧D again opened the same Note; the format `[Week ]GGGG-WW` typed in Settings showed `Journal/Week 2026-44.md` as typed, and ⌘⇧D, pressed with Settings in front, created and opened it in the Vault's window.
