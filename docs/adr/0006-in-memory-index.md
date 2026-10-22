# The index of the Vault lives in memory and is rebuilt at every launch

Wikilinks, Backlinks, Tags, the Quick switcher and search all need an index of the Vault. Noteki builds it in memory, reading every Note when a feature first needs one, and keeps it current from the file system's events. Nothing is stored on disk: no database to migrate, nothing that can go out of step with the Vault, and nothing to write inside the Vault ([ADR 0002](0002-coexist-with-obsidian.md)). The Vault has 624 Notes and the budget is a few seconds for 5,000, which a full read meets.

## Considered options

- **SQLite with full-text search in Application Support**: faster on huge Vaults, but a schema, migrations and invalidation for a problem not measured yet. It is the way out if the startup budget is ever broken.

## Consequences

- Search is a scan of the Notes' text in memory, without ranking beyond matches in the name first.
- Built in steps: for Wikilinks (ticket 11) the index holds only the files' names, from the listing, so it reads no Note.
- Backlinks (ticket 15) read the Notes too, but keep no text: for each link, the file name it asks for and its line, 8 bytes, about 4 MB for 5,000 Notes with 45 links each. The text of the lines is read from the file when the Backlinks of a Note are asked for. The links are read when the right sidebar is first shown, not at launch: at launch they put the app over its memory budget (SPEC, Budget). Reading them takes about 1.5 s for 5,000 Notes, 16 s the first time after the files were generated.
- Tags and search (tickets 16–17) have the same choice to make: keep the Notes' text in memory, which a Vault of 5,000 Notes may not afford inside the budget, or keep a compact form and read the files when asked, as the Backlinks do.
- Memory grows with what the index keeps per Note: names and the places of links, a few MB for thousands of Notes, not their text.
