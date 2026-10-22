# 14: Quick switcher

**What to build:** ⌘O opens a box that finds Notes by part of their name, recent first. Enter opens in the current view, ⌘Enter in a new Tab once Tabs exist. A name that matches nothing offers to create the Note.

**Blocked by:** 11 (Vault index and clickable Wikilinks)

**Status:** ready-for-agent

- [x] ⌘O opens the box; typing filters fuzzily, recent Notes first
- [x] Enter opens the chosen Note
- [x] A name with no match offers creation, in the folder of the Note in front
- [x] Tested at Seam 1 for ranking

## Comments

Done. ⌘O opens the Quick switcher over the window. Matching and ranking are in NotekiCore and tested at Seam 1:
- First by how well the name matches: the name itself, then a name that starts with the typing, a word that starts with it, a name that holds it, and the typed letters in order.
- Case and accents don't matter.
- Within the same quality, the recently opened Notes come first, then the shorter name, then the order of Files.
- With nothing typed, the recent Notes come first, then all the others.
- A path or a name with `.md` finds its Note the same way a Wikilink would.

Recency is ranked within the same quality on purpose. If recency came first, a recent Note matching only loosely would sit above the Note whose exact name was typed. The SPEC now says this.

When no Note has the typed name, the last row offers to create it. The same rule as Unresolved links applies: a bare name goes in the folder of the Note in front, or the Attachment in front, and a path goes from the root. No row is offered for a name the Vault refuses (`:`, an Attachment's name, outside the Vault, an Excluded folder) or for one a Wikilink can't hold (`[]#^|`).

The editor gets the keyboard once the chosen Note is in front, so you can type at once. Escape, or a click outside, gives the keyboard back to whatever had it before (Files or the editor).

Matching 5,000 Notes takes under 8 ms in a Release build, once per change of the typing.

⌘Enter for a new Tab waits for Tabs (ticket 25).

Verified in the app with keystrokes:
- "retro" lists the three Retro Notes, the recent one first; Down and Return open the second.
- `Clients/Axo/Plan` opens that Note.
- "Budget", "Fresh idea" and "Contract notes" (with a PDF in front) are created beside what was in front, and typing goes into the new Note.
- Escape returns the keyboard to Files and to the editor.

Seen during the tests: the system autocorrection popup in the editor holds incoming events until a key is pressed (AppleScript timed out meanwhile). This is macOS behaviour from ticket 05's autocorrect, not from this ticket.
