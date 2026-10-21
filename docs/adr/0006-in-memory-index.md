# The index of the Vault lives in memory and is rebuilt at every launch

Wikilinks, Backlinks, Tags, the Quick switcher and search all need an index of the Vault. Noteki builds it in memory at launch, reading every Note, and keeps it current from the file system's events. Nothing is stored on disk: no database to migrate, nothing that can go out of step with the Vault, and nothing to write inside the Vault ([ADR 0002](0002-coexist-with-obsidian.md)). The Vault has 624 Notes and the budget is a few seconds for 5,000, which a full read meets.

## Considered options

- **SQLite with full-text search in Application Support**: faster on huge Vaults, but a schema, migrations and invalidation for a problem not measured yet. It is the way out if the startup budget is ever broken.

## Consequences

- Search is a scan of the Notes' text in memory, without ranking beyond matches in the name first.
- Memory grows with the size of the Vault's text: a few MB for thousands of Notes.
