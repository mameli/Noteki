# 04: Write and save a Note

**What to build:** The editor becomes editable. Noteki saves the Note by itself about one second after the last change, and when the Note is closed or the app quits. There is no Save command. This introduces the open-note handle of the Vault module, which keeps the base text and decides what to write.

**Blocked by:** 01 (Open a Vault and read a Note)

**Status:** ready-for-agent

- [ ] Typing in a Note saves it about one second after the last keystroke
- [ ] Switching Note or quitting saves pending edits first
- [ ] The file is written atomically, keeping its line endings (LF or CRLF) and the presence or absence of a final newline
- [ ] Tested at Seam 1 through the open-note handle: edits produce the expected file on disk
- [ ] A contract test checks that the bridge's message names match between the Swift and TypeScript sides (deferred from 01, where only two messages existed)
