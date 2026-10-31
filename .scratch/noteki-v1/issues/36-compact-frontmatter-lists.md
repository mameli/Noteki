# 36: Compact lists in the frontmatter

**What to build:** List values in the frontmatter (tags, aliases…) take less vertical space, e.g. as inline chips on one line.

**Status:** ready-for-agent

- [x] A YAML list in the frontmatter is drawn compactly, wrapping only when it doesn't fit

## Comments

From user feedback 2026-10-31.

Done. The Frontmatter's lines are 0.9 em with a 1.4 line height, drawn or written alike (nothing moves on entering, as ADR 0008 asks); items have tighter chips. Items stay one per line: joining lines would change the height on entering.

After trying it: a list (not `tags`, which has its chips) is drawn side by side, wrapping, in place of its lines while the cursor is out of the Frontmatter; a click on it enters the YAML at the first item. So the Properties change height on entering, an exception to ADR 0008 the user asked for.
