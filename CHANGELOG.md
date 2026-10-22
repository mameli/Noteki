# Changelog

## Unreleased

Milestone 1, read and write (tickets 01–10): open a Vault, Files with Excluded folders, images and PDFs, writing with autosave, External changes reloaded or merged, Conflicts, Followed renames and Deleted Notes, opening files and folders from the Finder and Takku, Loose files.

### Budget

`scripts/measure`, 22 October 2026, Release build, generated Vault of 5,000 Notes, MacBook (Apple Silicon, macOS 26):

| | Measured | Limit |
|---|---|---|
| App on disk | 1.5 MB | 20 MB |
| Cold start to a Note shown, opened from the Finder as Takku does (restoring the last Note is ticket 25) | 0.72–0.83 s | 1 s |
| Listing 5,000 Notes (the index is ticket 11; the window is usable meanwhile) | 0.17 s | 3 s |
| Memory at rest, one window, one Note | 126 MB | 150 MB |

Memory by process: Noteki 55 MB, WebKit's WebContent 51 MB, GPU 13 MB, Networking 7 MB. WebKit is 71 MB of the 126, as the SPEC's known risks expected. WebKit's processes are the ones that appeared while Noteki started, so another app starting a web view at the same moment would be counted too. The first launch of a freshly built app takes about 1.3 s, because macOS checks and registers the new binary.

### Budget at M2 (tickets 11–15)

`BACKGROUND=1 scripts/measure`, 22 October 2026, Release build, generated Vault of 5,000 Notes with 45 links each. Launched in the background, so the WebKit figures are not those of the M1 table, which were foreground: compare only with the same method. The last commit before Backlinks (ticket 14), measured the same way, is the baseline.

| | Ticket 14 | Ticket 15, sidebar hidden | Ticket 15, sidebar shown | Limit |
|---|---|---|---|---|
| App on disk | 2.0 MB | 2.0 MB | 2.0 MB | 20 MB |
| Cold start to a Note shown | 0.95 s | 0.76 s | 0.73 s | 1 s |
| Listing 5,000 Notes | 0.16 s | 0.22 s | 0.20 s | 3 s |
| Listing and reading the links | | | 1.5 s | 3 s |
| Memory at rest | 146 MB | 149 MB | 155 MB | 150 MB |

With the sidebar shown the memory is over by 5 MB: Noteki 75 MB (59 hidden), of which the links are about 4 MB and the sidebar about 10 MB. Reading the links the first time after a Vault is generated takes 16 s (macOS looks at new files); every later launch, 1.5–1.9 s. Hidden, the margin is 1 MB.
