# 04: Write and save a Note

**What to build:** The editor becomes editable. Noteki saves the Note by itself about one second after the last change, and when the Note is closed or the app quits. There is no Save command. This introduces the open-note handle of the Vault module, which keeps the base text and decides what to write.

**Blocked by:** 01 (Open a Vault and read a Note)

**Status:** ready-for-agent

- [x] Typing in a Note saves it about one second after the last keystroke
- [x] Switching Note or quitting saves pending edits first
- [x] The file is written atomically, keeping its line endings (LF or CRLF) and the presence or absence of a final newline
- [x] Tested at Seam 1 through the open-note handle: edits produce the expected file on disk
- [x] A contract test checks that the bridge's message names match between the Swift and TypeScript sides (deferred from 01, where only two messages existed)

## Comments

Done. Verified in the app on a demo Vault with keystrokes sent through accessibility: autosave within about 1.25 s; typing then switching Note at once keeps every key in the right file; ⌘Q saves; CRLF stays CRLF; a file without a final newline stays so; a file with mixed line endings comes out with the ones most of its lines use.

From the review, ahead of tickets 06–07: a save never writes over a file another app changed since Noteki read or saved it. The edits stay in the editor, a banner says so, and switching Note or quitting asks: *Keep Editing*, *Discard My Edits* or *Replace With My Version*. Ticket 07's Merge replaces this question with a real merge. Late keys go to the Note just left through its own open-note handle; reports carry a revision so a stale one is ignored; switching Vault clears the editor.
