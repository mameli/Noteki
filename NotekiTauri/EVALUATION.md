# Evaluation of the Tauri prototype

Reproduce with `scripts/eval.sh --mutants`. It writes `src-tauri/target/eval-report.md`.

## What changed from the first Tauri prototype

| # | Problem | Effect | Now | Caught by |
|---|---|---|---|---|
| 1 | A reload refused by the editor (the user was typing) was forgotten | The next save **overwrote** the other app's change, for example Takku writing a meeting note while you type | Merged again into the newest text (`reload_rejected`) | `session::a_reload_refused_by_the_editor_…`, mutant 2 |
| 2 | Saving on close relied on `beforeunload`, which is not awaited | Up to ~1 s of typing could be lost on ⌘Q or closing the window | The window flushes on `onCloseRequested` and on `app-will-quit`; the backend flushes what it has on exit | review; see "Not evaluated" |
| 3 | `.lock().unwrap()` in every command | One panic and every later command panicked: the app was dead | Poisoned locks are recovered | `lib::tests`, mutant 9 |
| 4 | A rename by another app (Takku does one after Processing) showed as "deleted" | SPEC: Followed rename | The tab follows the note | `session::a_rename_…`, mutants 6 and 8 |
| 5 | During a conflict, later disk changes were ignored; *Keep mine* / *Take theirs* used text sent by the window | The banner could act on stale text | `NoteDoc` state machine (shared with GPUI) | unit tests, `session` |
| 6 | `save_note` wrote whatever text the window sent | A stale timer could write an old text | The backend saves the text last reported | `session` tests |
| 7 | Corrupt preferences were silently overwritten | Lost settings | Kept aside as `.bak` | `session::settings_…` |
| — | No search, tags or backlinks | | In-memory corpus updated on save and by the watcher | `session`, `demo_vault`, `perf` |

## How it is evaluated

| Level | Tool | Notes |
|---|---|---|
| Rules | `cargo test --lib` (27) | `NoteDoc`, scanner, index, frontmatter, wikilinks, merge, poisoned lock |
| Backend as the window drives it | `tests/session.rs` (19) | On a copy of `docs/demo-vault`: saving, echoes of our own writes, reload, merge, conflict, deleted, followed rename, refused reloads, links, search/tags/backlinks after saves and external edits, settings, **the JSON shape of every event `app.ts` reads** |
| Real data | `tests/demo_vault.rs` (5) | Every wikilink of the demo resolves; tags, backlinks, search |
| Robustness | `tests/robustness.rs` (6), 20k cases per property | No panic and valid ranges on generated Markdown (the watcher thread scans every changed file); `NoteDoc` invariants; merge laws |
| Frontend | `tsc` strict + esbuild build | Types of `app.ts` against the editor bridge and Tauri APIs |
| Speed | `examples/perf.rs`, 5,000 notes | |
| The tests themselves | `scripts/mutants.sh` (10 mutants) | Its first run found two weak tests of mine (mutants 3 and 6 survived); both are fixed |
| The binary | smoke run | The release app starts on a copy of the demo vault, the web view opens a note through the backend (seen in the saved preferences), an external edit does not crash it |

## Results (2026-11-16, macOS 27.0.1, Apple Silicon)

All checks pass: fmt, clippy `-D warnings`, 27 + 19 + 5 + 6 tests, tsc, build, perf, smoke, mutants 10/10.

| Operation on 5,000 notes (7 MB) | Time | Budget |
|---|---:|---:|
| Open vault, warm cache | 578 ms | 1500 ms |
| External change: rescan, re-read 1 note | 69 ms | 500 ms |
| Search, two words / `tag:` + `path:` | 32 / 38 ms | 200 ms (SPEC) |
| Backlinks of a note | 32 ms | 250 ms |
| Tag tree, rebuilt after each save | 60 ms | 250 ms |

## Not evaluated yet

- **Windows and Linux.** This is the reason for Tauri, and nothing has been built or run there yet. The code avoids macOS-only paths: `/` joins, atomic rename (which replaces on Windows), the config folder from Tauri. The next step is a CI matrix (`windows-latest`, `ubuntu-latest` with webkit2gtk, `macos-latest`) running `cargo test` and `tauri build`.
- **`app.ts` behaviour.** Only types and build are checked. Flushing on close, the refused-reload round trip and the new panes have no automated test; a WebDriver test (`tauri-driver`, Windows and Linux only) or Playwright on the web build would cover them.
- Memory, on any platform.
