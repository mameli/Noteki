# 17: Search

**What to build:** ⌘⇧F searches the Vault for words in any order, all required, ignoring case and accents. `tag:` and `path:` narrow the results. Results are grouped by Note, name matches first, each with its matching lines.

**Blocked by:** 16 (Tags pane)

**Status:** ready-for-agent

- [x] Words match in any order, case and accents ignored
- [x] `tag:x` and `path:Folder/` narrow the results
- [x] Results grouped by Note, name matches first, lines shown
- [x] Clicking a line opens the Note there
- [x] Tested at Seam 1

## Comments

Done. ⌘⇧F (Edit → Find → Search in Vault) shows the Search pane, the third of the left sidebar, with its field ready, and brings the Vault's window and its sidebar back if needed. A search runs a moment after the typing stops.

- Every word must be in the Note's name or text, in any order, case and accents ignored (`strasse` finds `Straße`). Double quotes keep a phrase or a path with spaces together.
- `tag:` uses the Tags pane's rules, sub-Tags included and `#` optional. `path:` keeps the Notes whose path holds it, anywhere in the path. With only `path:`, no file is read.
- Results are grouped by Note: those whose name holds every word first, then by path, each with up to ten lines and a count of the others. The first 300 Notes are listed, then a count.
- A click on a Note opens it. A click on a line opens it with the cursor on that line, also in the Note already in front, through a new editor command `goToLine`.
- Nothing is kept in memory (ADR 0006 records it): each search reads the files, about 0.2 s for 5,000 Notes. Results are found again a moment after a file changes, and when the pane is shown again.

Seam 1 tests cover the words, line numbers, names first, `tag:`, `path:`, quotes, limits, long lines cut around the word, ß, unreadable Notes, and a stopped search.

Fixed after review: a running search was not stopped by a newer one, results went stale while the pane was hidden, `tag:client/` found nothing, a long line with ß was cut in the wrong place, `path:` alone read every file, ⌘⇧F from a Loose file's window changed a hidden window, and a collapsed sidebar stayed hidden.

Verified in the app by typing: ⌘⇧F, results with their lines, a click on a line opening the Note there and a click in the Note in front moving the cursor, `tag:` and `path:` narrowing, and a Note written on disk appearing when the pane came back.
