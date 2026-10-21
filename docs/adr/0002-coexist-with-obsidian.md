# Noteki opens an Obsidian vault as it is and leaves it as Obsidian expects

The user keeps one **Vault** (`~/Documents/Agile`) and still needs Obsidian for what Noteki will never do: git through obsidian-git, Excalidraw, the occasional plugin. Both apps must work on the same folder, in turn or at the same time. Noteki therefore writes nothing in the Vault but Notes and Attachments: never in `.obsidian/`, no index or cache file, no settings. Its settings and anything it remembers live in `~/Library/Application Support/Noteki/`. What it writes in a Note follows Obsidian's format: Wikilinks by name, Embeds as `![[file]]`, Tags in the `tags` Property, Pasted images named `Pasted image YYYYMMDDHHmmss.png`.

## Consequences

- Wikilinks resolve by Obsidian's rules, not by rules that might be nicer: a Note that works in one app works in the other.
- Noteki does not read Obsidian's settings either (excluded files, attachment folder, Daily note folder). The ones it needs are set again in Noteki, once.
- Obsidian open on the same Vault is an **External change** like any other ([ADR 0005](0005-merge-external-changes.md)).
