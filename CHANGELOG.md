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

### Budget with the Tags pane (ticket 16)

`BACKGROUND=1 scripts/measure`, same Vault and method, run in turn on the same build:

| | Default (Files, sidebar hidden) | `SIDEBAR=1` | `TAGS=1` | Limit |
|---|---|---|---|---|
| Noteki's own process | 59 MB | 70 MB | 60 MB | |
| Memory at rest, all processes | 108 MB | 117 MB | 109 MB | 150 MB |
| Cold start to a Note shown | 0.84 s | 0.89 s | 0.90 s | 1 s |
| Listing 5,000 Notes | 0.30 s | 0.26 s | 0.26 s | 3 s |
| Listing and reading links / Tags | | 2.2 s | 2.0 s | 3 s |

Correction to the M2 table above: WebKit's processes were 60 to 69 MB of the totals there, and are 30 MB in this run, on the same method. The WebKit figure varies from run to run on this Mac (other apps, the state of WebKit's caches), so the totals of the two tables are not comparable. Noteki's own process is the steady figure: 55 MB at ticket 14, 59 MB now, 70 MB with the right sidebar shown. At worst WebKit's 69 MB, which has been seen, makes the sidebar-shown total 155 MB. Reading the links of 5,000 Notes is 1.5 to 2.2 s, slower than before the reading was shared with Tags (1.5 s); the Tags take 1.8 to 2.0 s.

### Search (ticket 17)

Search keeps nothing in memory: each search reads the files. On 5,000 generated Notes, Release build, files in macOS's cache: 0.19–0.24 s, whether it finds nothing, two Notes or all 5,000 (75,000 lines, of which the first 300 Notes and 10 lines each are listed). A search typed over is stopped where it is, not only its answer dropped.

### Outline (ticket 18)

`BACKGROUND=1 scripts/measure`, same Vault, one run each:

| | Default | `SIDEBAR=1` (Outline) | `BACKLINKS=1` | Limit |
|---|---|---|---|---|
| Noteki's own process | 60 MB | 70 MB | 76 MB | |
| WebKit's processes | 90 MB | 80 MB | 80 MB | |
| Memory at rest, all processes | 150 MB | 150 MB | 156 MB | 150 MB |
| Cold start to a Note shown | 0.80 s | 0.85 s | 0.84 s | 1 s |

In these runs WebKit was at its high (WebContent 60–69 MB, against 30 MB in the Tags runs), which puts even the default at the limit. Noteki's own process at first launch went from 55 MB (ticket 14) to 60 MB over tickets 15–18. The memory budget is the open question of M2: it holds or not depending on WebKit's state.
