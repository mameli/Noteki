# 12: A real app: bundle, icon, opening files from the Finder and Takku

**What to build:** `scripts/bundle.sh` makes `Noteki GPUI.app` (Info.plist, icon from the Swift app's assets, the release binary), declaring the Editor role for Markdown and the Viewer role for folders as SPEC › Takku says. Opening a file (`cx.on_open_urls`): a Note or Attachment of the Vault shows in a Tab; a folder of the Vault is selected in Files; a Markdown file outside the Vault opens as a **Loose file** in a window of its own, saved like a Note.

**Blocked by:** none

**Status:** done

- [x] The bundle builds; Launch Services registers it with the Editor role for Markdown and the Viewer role for folders (not launched in the foreground: it would take the keyboard)
- [x] Loose file window: one Tab, no sidebars, saved and merged as a Note
- [x] Tests: the routing of an opened path (Vault Note, Attachment, folder, outside) without GPUI

## Comments

- `scripts/bundle.sh` builds `target/release/Noteki GPUI.app` (19 MB, ad-hoc signed, id `app.noteki.noteki-gpui`, icon from the Swift app's iconset), registers it with Launch Services; `--open FILE` opens a file with it.
- `on_open_urls` hands the URLs through an async channel to the window, so files given at launch wait for it (no polling).
- `core::opening::route`: a Vault file goes to a new Tab (or the one showing it), a Vault folder is selected in Files, a Markdown/text file outside is a Loose file, another folder asks whether to switch Vault (opened directly when there is none), the rest only brings the window forward.
- The Loose window (`app/loose.rs`) saves atomically a second after the last change, on deactivation and on close, and merges with the disk when it comes back to the front (same `NoteDoc` rules as a Note). No file watcher: YAGNI until it proves needed.
- To check by hand: `NotekiGPUI/scripts/bundle.sh --open "some note.md"`.
