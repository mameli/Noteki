# 13: Wikilink completion

**What to build:** Typing `[[` opens a list of Notes, fuzzy on the name, the most recently opened first. Choosing one writes its name, or the shortest unique path when the name is ambiguous, and closes the brackets.

**Blocked by:** 11 (Vault index and clickable Wikilinks)

**Status:** ready-for-agent

- [x] `[[` opens the list; typing narrows it fuzzily
- [x] Recently opened Notes come first
- [x] An ambiguous name is written as the shortest unique path
- [x] Tested at Seam 1 for the candidates and the text inserted

## Comments

Done. Typing `[[` makes the editor ask the app for the Notes, once per link. The app answers with the recently opened Notes (latest first) and then the others. CodeMirror's completion filters fuzzily on the name as the user types. Recent Notes get a boost under 100, so they come first among names that match equally well, and an exact match is not pushed below a weaker match on a recent Note.

What is written: the name, or the fewest folders before it that only this Note's path ends with, checked against the same resolution as clicking (Obsidian 1.14.4). A test checks that every link written leads back to its Note from every Note. Obsidian itself writes the whole path as soon as a name is shared. Here the link is shorter but leads to the same Note. A Note such as `Report.pdf.md` next to `Report.pdf` gets a path, so its link doesn't lead to the PDF.

The brackets are closed. Inside a link already closed, choosing replaces the name and keeps the Alias or heading. No list appears in code.

Recently opened Notes are remembered per Vault in the app's preferences (50 at most) and follow a Followed rename.

For 5,000 Notes with mostly shared names: 33 ms to build the list and 8 ms for the JSON, Debug build, once per `[[`.

Verified on the editor page in a browser, with the app's answer simulated: the list, recent first, fuzzy narrowing, a shared name written as a path, an existing link with an Alias, a closed link without one, and inline and fenced code. In the real app only the build and the remembered order were checked: typing in its window was skipped because the user was working in another app.
