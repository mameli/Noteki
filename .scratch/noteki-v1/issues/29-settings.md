# 29: Settings

**What to build:** A Settings window for the text font, its size, the maximum line width and the user's own Excluded folders.

**Blocked by:** 02 (Excluded folders), 04 (Write and save a Note)

**Status:** ready-for-agent

- [x] Font, size and line width apply to open Notes at once
- [x] Added Excluded folders disappear from the Files pane and the index
- [x] Settings are stored outside the Vault
- [x] The editor's settings are injected into editor.html before it loads, with no round trip (ADR 0004)

## Comments

Done. *Noteki → Settings…* (⌘,) has two panes.
- **Editor**: the text font (any family installed, the system font by default), its size (10–32 points, 15), the line width (480–1,600 points, 736 as the 46 rem before), and Restore Defaults. A point is a CSS pixel of the editor.
- **Excluded Folders**: the folders of the first window's Vault the user leaves out, chosen inside the Vault (one already left out is refused, said so), removed with −. Kept per Vault, by path from its root, matched in any letter case.

Everything is kept in the app's preferences, outside the Vault. A change reaches every window's editor at once, Loose files' too, and every window of that Vault: Files is listed again, so the index, links, Tags and Search leave the folder out. A folder newly left out closes the Tabs inside it, their Notes saved first: no change on disk would reach them any more (ADR 0005).

The page gets the settings before it loads, from a user script at document start (ADR 0004): the first Note is drawn with them, with no round trip. They are sent again once the editor is ready, for a change made while it loaded; after a font change CodeMirror measures its lines again, and once more when the font has loaded.

Tests: Seam 1 (folders the user excludes in any case, with what they hold, for changes, openings, new Notes and renames; the settings' defaults, bounds and JSON) and Seam 2 (the CSS variables, the system font, a font name that cannot break out of its quotes).

Not done, in the SPEC: an excluded folder renamed or moved outside Noteki is no longer excluded; Settings shows the first window's Vault.

Fixed after review:
- a Note open in a folder newly left out would have missed changes on disk and could have overwritten them;
- the Excluded Folders pane kept the list of the Vault left;
- a folder already left out was refused as "not inside the Vault";
- a change made while the page loaded was lost;
- a new font could be measured before it loaded;
- the font menu drew a thousand families each in its own font.

Verified in the app, on the demo Vault: size 20, Georgia and a 576-point width applied at once to the open Note (Kickoff, then Long); Archive added through the folder panel left Files at once; after quitting and launching again the first Note was drawn in Georgia 20 at 576 points, Archive still left out; removing Archive brought it back; Restore Defaults put back the system font at 15 and 736. Not tried: several windows at once, a Loose file's window, a Note open in a folder being left out.
