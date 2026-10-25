# Properties are read from a subset of YAML, by Noteki, and drawn in place

The Properties table needs the Frontmatter read: which keys, which values, which lists. NotekiCore owns Frontmatter parsing (SPEC), and Noteki must never rewrite YAML it does not understand. A full YAML library would read anything, but it is a dependency several times the size of the code that uses it. It would also turn a Note's Frontmatter into values that no longer say where they are written, while the table must be drawn over the YAML itself.

Noteki reads the YAML that Obsidian and Takku write, by hand, in NotekiCore:
- `key: value` at the margin, the value plain or in quotes;
- a list one item per line, or in brackets on one line;
- blank lines and comments.

Every key, value and item comes with where it is written: a line and its UTF-16 columns, as the editor counts them. Anything else is YAML Noteki cannot read: a map inside a Property, a block of text, anchors, a value over several lines, a key twice. It is shown as written, with a mark on the line where reading stopped, and never rewritten. All 159 Frontmatters of the user's Vault read.

The editor asks the app to read its Frontmatter whenever it changes (`readFrontmatter`). It draws the table only while the Frontmatter is still the text the answer was about. As tables are ([ADR 0008](0008-tables-drawn-in-place.md)), the table is drawn in place: each line of YAML stays one line of the table, never wrapped, so the text does not move up or down when the cursor enters the YAML or leaves it.

## Considered options

- **A YAML library** (such as Yams, built on LibYAML): reads all of YAML, but gives values without where they are written, and is a dependency for the small part the Vault uses.
- **Reading in the editor**, in TypeScript: no round trip, but Frontmatter parsing would leave NotekiCore, and ticket 24 writes Tags back there.
- **A block of its own** (a table replacing the YAML): it would follow Obsidian's look, but its height differs from the YAML's, and the text would jump when the cursor entered it.

## Consequences

- A list in brackets on one line shows its items side by side on that line, not one per line.
- A quoted value shows what is between its quotes as written: `\"` and `''` are not undone.
- Valid YAML outside the subset is shown as source with the error mark, as if broken. Obsidian would read it.
- Until the app answers about the Frontmatter as it is now, it is shown as written: same height, so nothing jumps when the table arrives.
