# 12: Unresolved links

**What to build:** Wikilinks to Notes that do not exist are drawn in another colour. Following one creates the Note in the folder of the Note that holds the link and opens it.

**Blocked by:** 11 (Vault index and clickable Wikilinks)

**Status:** ready-for-agent

- [x] Unresolved links look different from resolved ones
- [x] Following one creates `<name>.md` in the linking Note's folder and opens it
- [x] The link turns resolved as soon as the Note exists
- [x] Tested at Seam 1

## Comments

Done. The editor sends the app the Note's links. The app answers with those that lead nowhere, and answers again whenever the index changes, so a link turns resolved once its Note exists.

Unresolved links are drawn faded, as in Obsidian, and so is an Embed of a missing file. Following one creates an empty Note and opens it.

Where it goes, checked in Obsidian 1.14.4's code (`createNewMarkdownFileFromLinktext`):
- A bare name goes in the linking Note's folder. That is our choice; Obsidian's default is the Vault's root.
- A path goes from the root, as in Obsidian.
- Names with `\` or `:` are refused, as in Obsidian on a Mac.
- Never over an existing file. A file already there under other letter case is opened instead.

Verified in the app with real clicks: [[Kickoff]], a path with new folders, an Embed of a missing image (which only beeps), a link typed afterwards, and a link at line 1990 of a 2,000-line Note.
