# 12: A real app: bundle, icon, opening files from the Finder and Takku

**What to build:** `scripts/bundle.sh` makes `Noteki GPUI.app` (Info.plist, icon from the Swift app's assets, the release binary), declaring the Editor role for Markdown and the Viewer role for folders as SPEC › Takku says. Opening a file (`cx.on_open_urls`): a Note or Attachment of the Vault shows in a Tab; a folder of the Vault is selected in Files; a Markdown file outside the Vault opens as a **Loose file** in a window of its own, saved like a Note.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] The bundle builds and launches; `open -a` with a Note shows it
- [ ] Loose file window: one Tab, no sidebars, saved and merged as a Note
- [ ] Tests: the routing of an opened path (Vault Note, Attachment, folder, outside) without GPUI
