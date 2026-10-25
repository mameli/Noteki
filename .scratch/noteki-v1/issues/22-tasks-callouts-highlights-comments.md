# 22: Tasks, callouts, highlights and Comments

**What to build:** Task list checkboxes are clickable and write `[x]` or `[ ]` in the file; callouts show their title and colour; `==highlight==` renders; Comments (`%% … %%`) are hidden outside the cursor.

**Blocked by:** 19 (Live preview of inline syntax)

**Status:** ready-for-agent

- [x] Clicking a checkbox toggles it in the file
- [x] Callouts render with title and colour, without folding
- [x] Highlights render
- [x] Comments are hidden unless the cursor is inside them
- [x] Tested at Seam 2

## Comments

Done.
- **Tasks:** a checkbox in place of the bullet and brackets (a number stays); they show while the cursor touches them. A click writes `[x]` or `[ ]` in the file as an ordinary edit (⌘Z undoes it, the app saves it as it saves typing), so no bridge message was needed. The cursor stays where it was. It answers the click VoiceOver sends too. A done task is crossed out and faded, as in Obsidian's default theme.
- **Callouts:** a box in the colour of Obsidian's type (aliases included, any other type a note's), with the written title or the type with a capital, always open. Their text is not a quote's grey. Links in quotes are now blue.
- **Highlights:** `==text==` with a background; `==` opens and closes as `~~` does.
- **Comments:** hidden line by line, lines kept, so Takku's `%% takku:start %%` leaves an empty line. In a paragraph a Comment closes at the next `%%` of that paragraph. A line starting with `%%` opens a Comment over blank lines and blocks, also right under a paragraph or a list.

Two choices keep the SPEC's rule that the text above the cursor never moves; they are in the SPEC.
- A callout does not turn wholly into source when the cursor enters it: the box stays, and its `>` and `[!type]` show on the cursor's line only, as an inline element's marks.
- A Comment across lines shows from the cursor's line down, so entering it from below shows its last line first.

Tests: Seam 2 (what hides, the checkboxes and what a click writes, the callouts' kinds and titles, Comments in paragraphs, lists, quotes and code, and the cursor moving without changing the lines above).

Not done, in the SPEC: Obsidian's `[-]` or `[/]` tasks stay as written (only `[ ]`, `[x]` and `[X]`, as on GitHub); a callout inside a quote stays as written; one in a list has its box under the bullet too; text after the closing `%%` of a Comment of lines stays as written.

Fixed after review:
- a `%%` line right under a list was not a Comment, and its closing `%%` then hid the rest of the Note;
- a paragraph's Comment across lines hid the `>` and indent of a quote or a list;
- a callout in a list item kept its first `>`.

Verified on the editor page in the browser: the four elements drawn, a Comment entered from below shown a line at a time with nothing above moving, line heights unchanged, the checkbox writing `[x]`. Verified in the app, launched in the background and driven through accessibility while Claude stayed in front: Takku's markers hidden, the highlight, the checkboxes and the callout's title drawn. Pressing each checkbox wrote `[x]` and then `[ ]` into the file on disk. Not looked at in the app window: the colours and the fixes after review.
