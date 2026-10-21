# 09: Takku integration

**What to build:** Noteki declares the Editor role for Markdown and plain text, and folders, so Takku lists it among its Editors and hands it the Vault for *Show all*. A Markdown file outside the Vault opens in its own window without indexing anything around it; a folder that is not the Vault asks whether to switch Vault.

**Blocked by:** 01 (Open a Vault and read a Note)

**Status:** ready-for-agent

- [x] Noteki appears in Takku's Editor choice
- [x] Opening a Note of the Vault from Takku or the Finder shows it in Noteki
- [x] Opening a Markdown file outside the Vault opens it alone in its own window
- [x] Opening the Vault folder or one inside it shows it in the Files pane
- [x] Opening another folder asks whether to switch Vault

## Comments

Done. Takku's own logic from `EditWithMenu.swift`, run against the built app, lists Noteki as a Markdown Editor and sees that it opens folders. Folders have the Viewer role, which Takku ignores. Plain text is not declared, because that type covers every source file.

Verified in the app with `open -a`:
- A Vault Note at cold start is shown. Opening a file never changes the remembered Vault.
- An Attachment is shown, and a Vault folder is selected.
- A file in an Excluded folder only brings the window forward.
- A Markdown file from outside opens as a Loose file in its own window, saves, and closes once after saving, even with two ⌘W.
- A PNG from outside is ignored.
- Another folder asks whether to switch Vault.

A Note Takku has just written is selected once Files lists it. A Loose file's folder is watched, but only events in that folder itself are looked at.
