# NotekiGPUI

A prototype of Noteki in Rust on [GPUI](https://www.gpui.rs) with [GPUI Kit](https://gpui-kit.com) 0.10. It answers one question: could GPUI replace SwiftUI + CodeMirror for Noteki? It reads and writes the same Obsidian vault as the Swift app. See [EVALUATION.md](EVALUATION.md) for the answer and the evidence.

```sh
cargo run                      # opens the last vault, or docs/demo-vault
scripts/eval.sh                # every check, report in target/eval-report.md
scripts/eval.sh --mutants      # plus the mutation check of the tests
```

`NOTEKI_VAULT=/path/to/vault` opens another vault. `NOTEKI_SETTINGS=/tmp/s.json` uses another preferences file (the default is `~/Library/Application Support/Noteki/gpui-settings.json`).

## Layout

| Path | What | Depends on GPUI |
|---|---|---|
| `src/core/vault.rs` | Scanning, atomic writes, create/rename/copy/trash, daily-note paths | no |
| `src/core/wikilink.rs` | Wikilink parsing, Obsidian resolution, link completions | no |
| `src/core/markdown.rs` | The only Markdown scanner: blocks, inline tokens, live-preview spans, the HTML conversion for Export | no |
| `src/core/index.rs` | `Corpus` of note texts; outline, tags, backlinks, search, quick switcher | no |
| `src/core/document.rs` | `NoteDoc`: saving, Merge, Conflict, Deleted (SPEC › External changes) | no |
| `src/core/tabs.rs` | Tabs, per-tab Back/Forward, closed tabs, rename/remove | no |
| `src/core/editor/` | The editor's rules: text with selection and composed text, movement by grapheme/word/line, UTF-16 for macOS input, line geometry for lines of different heights, Live preview styles and Heading levels per line, the display map (marks hidden by Live preview), code colours (syntect), tables, undo history, format commands | no |
| `src/core/merge.rs`, `frontmatter.rs`, `embeds.rs` | Merge, Frontmatter and embeds, ported from NotekiCore | no |
| `src/editor/` | `NoteEditor`, Noteki's own editor element: shapes and paints each line on its own, keys, mouse, native text input (replaces GPUI Kit's `Editor`, see `.scratch/gpui-editor/`) | yes |
| `src/app/mod.rs` | `NotekiApp`: state, file watching, wiring of `core` to GPUI | yes |
| `src/app/render.rs` | All drawing, with stable element IDs for UI tests | yes |
| `src/app/settings.rs`, `src/editor/style.rs`, `src/theme.rs` | Preferences and per-vault sessions; live-preview styles; Ayu / Everforest / One Dark Pro | yes |

The rules live in `core` and are tested there; `app` only connects them to the window and the file system.

## Tests

| Suite | Command | What it proves |
|---|---|---|
| Unit | `cargo test --lib` | The rules of each `core` module and the settings |
| Regressions | `cargo test --test regressions` | The two crashing/broken behaviours of the first prototype stay fixed |
| Robustness | `NOTEKI_FUZZ_CASES=20000 cargo test --release --test robustness` | No panic and valid UTF-8 ranges on generated Markdown; `NoteDoc` and `Tabs` invariants under random operations; merge laws |
| Demo vault | `cargo test --test demo_vault` | On the real `docs/demo-vault`: every wikilink resolves, the HTML conversion parses and every link in it works; tags, backlinks, outline, search |
| UI | `cargo test --test ui` | The real `NotekiApp` in a headless GPUI window: typing and autosave timing, reload/merge/conflict/deleted banners, followed rename, links, tree, rename/trash, session restore, CRLF |
| Performance | `cargo run --release --example perf` | Budgets on a 5,000-note synthetic vault |
| Keystroke timing | `cargo test --release --test keystroke_timing -- --ignored --nocapture` | Time per keystroke and per idle frame of the real `NotekiApp` on a 100 KB Note (informational, no budget) |
| Mutation | `scripts/mutants.sh` | The tests catch each fixed bug when it is put back |
