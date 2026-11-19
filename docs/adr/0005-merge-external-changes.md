# External changes are merged with the user's edits, not overwritten in either direction

Takku rewrites the Meeting note after the stop, often while the user is still writing their Personal notes in it, and Obsidian may be open on the same Vault. Reloading the file would lose what the user typed in the last second; keeping the editor's text would erase Takku's Summary at the next save. Noteki saves by itself about one second after the last keystroke and, on an **External change**:

1. if everything in the editor is saved, it reloads the Note silently, keeping the cursor where it was in the text;
2. otherwise it does a three-way **Merge**, with the last version Noteki saved or loaded as the common base. Takku rewrites only its own part of the Note, so the two sides rarely touch the same lines;
3. on a **Conflict** it keeps the user's text in the editor, does not save over the file, and shows a banner with the difference: the user chooses which version to keep or edits by hand.

## Considered options

- **The file on disk always wins** (reload): loses notes written during a Meeting.
- **The editor always wins**: erases what Takku wrote.
- **Always ask**: interrupts the user at every Summary, which is exactly the moment they are writing.

## Consequences

- Noteki keeps the base version of every open Note in memory.
- Noteki hears of an External change only once the file has been quiet for 250 ms (`FS_SETTLE`), so each save reads the file first. When the file is no longer the base, the save handles it as an External change before it writes (20 November 2026: until then a save in that quarter of a second wrote over Takku's Summary, a path found with a TLA+ model of this flow). Another app can still write between that read and the write, a few microseconds. Only a lock shared by the apps would close that gap.
- Saving and merging are the riskiest code of the app. They are tested on real sequences of Takku's writes in `src/core` (`merge.rs`, `document.rs`, `opening.rs`), and in `tests/robustness.rs`.
