# Each window of the Vault is a model of its own, and Tabs keep their Notes open

⌘⇧N opens another window of the Vault, with its own web view ([ADR 0003](0003-one-web-view-per-window.md)). Until ticket 25, one model held both what is the Vault's (Files, the index, the links and Tags read, Search, the recent Notes, the watcher on the folder) and what is the window's (the Note in front, its saving, Conflicts and Followed renames, the sidebars, the Quick switcher). Splitting it in two would touch nearly every line of the app's model and every view.

Each window is a model of its own instead, on the same folder. A second window lists the Vault again (0.2 s for 5,000 Notes), keeps its own index of names (about a megabyte), and reads the links or the Tags only when one of its panes asks for them. The same Note open in two windows is two open Notes on one file. What one window saves, the other sees as a change on disk: it reloads it, or merges it with what the user typed there, exactly as with Takku or Obsidian ([ADR 0005](0005-merge-external-changes.md)). The recent Notes are read from the preferences before each one is added, so the windows share them.

Within a window, each Tab keeps its Note open. The Note of the Tab in front is in the editor. The Notes of the Tabs behind keep their text in the app, saved when their Tab was left, and their editor state in the web view (cursor, scroll, undo). A Note behind follows a Followed rename, and takes the file's changes, which reach the editor when its Tab comes back. A Note that a Tab no longer shows is let go, in the app and in the editor.

## Considered options

- **One model of the Vault shared by its windows' models**: no work done twice, but the whole app model and its views to split, for savings that are small next to a web view (40–90 MB a window).
- **Tabs that read their Note again when they come back**: simpler, but each Tab would lose its undo, and a Tab whose Note was renamed while behind would lose its file.

## Consequences

- A second window costs its web view and a second listing of the Vault; Tabs cost only their text.
- Choosing another Vault in any window applies to the first one; the other windows close, as they showed the Vault left.
- The Tabs of every window are kept in the app's preferences, per Vault, the first window's first. They come back at launch, without the Tabs whose file is gone, and the other windows open again with theirs.
