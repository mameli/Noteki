# Evaluation of the GPUI prototype

**Question.** Can Noteki be built on GPUI + GPUI Kit 0.10 instead of SwiftUI + CodeMirror, while keeping the behaviour in [SPEC](SPEC.md)?

**Short answer.** Yes for the app shell, files, saving and merging, links, search and the Reader. Not yet for the editor: GPUI Kit's `Editor` can colour and fade Markdown, but cannot hide marks or draw images, tables or formulas inline. Those are what Noteki's CodeMirror live preview does. One upstream crash (markdown-rs) has to be guarded, as this prototype does.

Run `scripts/eval.sh --mutants` to reproduce everything below. It writes `target/eval-report.md`.

## How it is evaluated

| Level | Tool | Why this level |
|---|---|---|
| Rules | 44 unit tests in `src/core`, `src/app/settings.rs` | Saving, merging, tabs and links are pure functions or state machines; their cases are cheap to state exactly |
| Robustness | `tests/robustness.rs`: deterministic fuzzing, 20k–50k cases per property | The editor applies scanner ranges on every keystroke, so one bad offset crashes the app. Checks: no panic, every range on a char boundary, Reader output keeps line count, `NoteDoc`/`Tabs` invariants, merge laws |
| Real data | `tests/demo_vault.rs` on `docs/demo-vault` | Every wikilink of the shipped vault resolves; the Reader Markdown, parsed with **the same options as GPUI's TextView**, contains a working link for every wikilink and an existing file for every image |
| Whole app | `tests/ui.rs`: 21 headless GPUI tests of the real `NotekiApp` | Clicks, typing and simulated time: autosave timing, banners, tree, Reader links and checkboxes, rename/trash, session restore, CRLF |
| Speed | `examples/perf.rs`, 5,000 notes / 7 MB | Per-keystroke work within a frame; vault-wide work well under SPEC's 0.2 s for search |
| The tests themselves | `scripts/mutants.sh` | Puts each fixed bug back, one at a time; the suite must fail. A mutant that does not compile counts as a failure of the check, not as killed |
| The binary | smoke run in `eval.sh` | The release app starts on a copy of the demo vault, survives an external edit, persists its session, prints no panic |

## Results (2026-11-16, macOS 27.0.1, Apple Silicon)

| Check | Result |
|---|---|
| fmt, clippy `-D warnings` | pass, 0 warnings |
| Unit / regressions / robustness / demo vault / UI | 44 / 2 / 11 / 9 / 21 passed, 0 failed |
| Mutation check | 10 / 10 mutants killed |
| Smoke run | pass |

| Operation on 5,000 notes | Time | Budget |
|---|---:|---:|
| Open vault, warm cache (scan, read, index, tags) | 575 ms | 1500 ms |
| External change: rescan tree, re-read 1 note | 69 ms | 500 ms |
| Search, two words / `tag:` + `path:` | 31 / 37 ms | 200 ms (SPEC) |
| Backlinks of a note | 31 ms | 250 ms |
| Tag tree, rebuilt after each save | 60 ms | 250 ms |
| Live preview spans, 100 KB / 1 MB note, per keystroke | 0.4 / 4.4 ms | 16 / 120 ms |
| Reader conversion, 100 KB note | 0.6 ms | 30 ms |

## What was wrong in the first prototype

Each bug was reproduced before it was fixed. The "Caught by" column names the check that fails if the bug comes back.

| # | Bug | Effect | Caught by |
|---|---|---|---|
| 1 | The scanner skipped 2 bytes after `\` | **Crash** on typing `\è`, `\€`, … | `regressions`, `robustness`, mutant 1 |
| 2 | Reader wikilinks written as `[x](noteki-link://Launch plan)` | Links with a space (nearly all of them) were plain text | `regressions`, `demo_vault`, mutant 2 |
| 3 | The tree and the row both toggled a folder on click | Folders could not be opened | `ui::clicking_a_folder_expands_it_once` |
| 4 | Every keystroke re-read every note from disk (backlinks); every autosave rescanned the vault and re-read all notes for tags | Typing cost grows with the vault | `perf` (now in-memory `Corpus`) |
| 5 | Reader checkbox used Reader offsets on the note's text | Ticked the wrong task when a link or comment came before it | `ui::reader_checkbox_toggles_the_right_task` |
| 6 | Autosave ignored "deleted on disk"; trashing saved the note after moving it | A note deleted by another app came back; a trashed note reappeared | `ui::deleted_note_…`, `ui::rename_and_trash_…`, mutant 5 |
| 7 | If moving to the Trash failed, the file was deleted for good | Data loss (SPEC: never deleted for good) | review; now an error |
| 8 | A clean Merge was not saved; a reload reset the cursor | SPEC › External changes not met | `ui::external_change_…`, mutant 9 |
| 9 | Own-write echoes ignored for 1 s by time | An external change in that second was lost | `NoteDoc` compares contents instead (unit tests) |
| 10 | Cursor moves emit no editor event | Live preview did not reveal the marks of the line being edited | `cx.observe` on the editor |
| 11 | Errors dropped (`let _ =`, `.is_ok()`) | Failed saves were silent | status bar; `ui` asserts its text |
| 12 | Unresolved links styled red and wavy; created `# Title` notes; daily notes in `Journal/Daily_YYYY` | Differs from SPEC and the demo vault | `ui::following_an_unresolved_link_…` |

The evaluation tools also caught **my own** mistakes: the fuzzer found a crash in the new scanner (mutant 3) and a `NoteDoc` state that stayed "unsaved" although the disk already had its text, and the mutation script once reused a stale mutated build because `mv` kept an old mtime. All three are fixed.

## Findings about GPUI / GPUI Kit

- **markdown-rs 1.0.0 panics on valid Markdown**, for example ``1. ```\n-``. It did so on about 1 in 1,000 generated notes. GPUI Kit's `TextView` parses without a guard, so such a note would crash the app in the Reader. `core::markdown::reader_guard` pre-parses with the same options inside `catch_unwind` and falls back to showing the source. This should be reported upstream.
- **Testing is good.** `TestAppContext`, simulated clocks and `gpui_kit::test` drive the real view headlessly in about 1.5 s for 21 tests. One snag: enabling `test-support` needed `cargo update -p wgpu --precise 30.0.1` to resolve.
- **Components that fit**: `TitleBar`, `Tree` (it owns expansion, so the app must follow its events), `TabBar`, `Command`, `Dialog`, `h_resizable`, `Input`, `TextView` (links, task toggles, images, frontmatter), and themes mapped from Noteki's palettes.
- **The editor is the limit.** `Editor` decorations are `HighlightStyle` only: colour, weight, italic, strike, background and `fade_out`. There is no per-range font size, no hidden text, and no inline blocks. Noteki's live preview hides marks, sizes headings, and draws images, tables, KaTeX and Mermaid inside the text; none of this is possible without a custom editor element.

## SPEC coverage

| Area | State in the prototype |
|---|---|
| Vault scan, excluded folders, atomic save after ~1 s, CRLF and final newline | done, tested |
| External changes: reload, Merge then save, Conflict (Keep mine / Take theirs), Deleted (Save again), Followed rename | done, tested. A rename is followed when the new file's content matches (no file-identity check); *Show differences* is missing |
| Wikilink resolution, unresolved → empty note next to the source | done, tested |
| Typing `[[` completion | missing; ⌘K opens a picker instead |
| Live preview | partial: marks faded, not hidden; no inline images, tables, math, diagrams; no heading sizes |
| Reader | done: links, embeds, checkboxes, callouts as bold titles. KaTeX and Mermaid are not drawn |
| Tabs, Back/Forward, closed tabs, session per vault | done, tested. Closing an only-on-screen note keeps it in memory instead of asking |
| Files pane | create, rename, copy, trash (to `~/.Trash`, no Put Back). No drag-and-drop or context menu |
| Quick switcher | uses `Command`'s own filter, not SPEC's ranking. No ⌘Enter, no create-on-missing |
| Tags, search, outline, backlinks | done, tested |
| Large notes | live preview off above 1 MB (SPEC: Source mode above 5 MB) |

**Not evaluated:** pixels (no screenshot tests), IME and accessibility, memory use, real FSEvents latency (UI tests call `handle_fs_changes` directly; only the smoke run uses the watcher), multiple windows.

## Recommendation

Keep `core` as it is; it is the reusable part. The decision depends on the editor. Before going further, build a spike of a custom GPUI editor element that hides marks and draws an image block, or test whether an embedded CodeMirror webview is acceptable. Without either, a GPUI Noteki would show Markdown source with colours, which is a step back from the Swift app.
