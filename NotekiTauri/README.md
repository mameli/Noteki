# NotekiTauri

The cross-platform prototype of Noteki (macOS, Windows, Linux): a Tauri 2 shell around the **same CodeMirror editor** as the Swift app (`../Editor`), with a Rust backend for the vault.

```sh
npm install                          # once, plus `npm install` in ../Editor
npx tauri dev                        # run
scripts/eval.sh                      # every check, report in src-tauri/target/eval-report.md
scripts/eval.sh --mutants            # plus the mutation check of the tests
```

`NOTEKI_VAULT=/path` opens a vault at launch; `NOTEKI_SETTINGS=/tmp/s.json` uses another preferences file.

## Layout

| Path | What |
|---|---|
| `src/app.ts`, `index.html`, `src/shell.css` | The window: file tree, Search / Tags panes, Backlinks, banners, the bridge to the editor |
| `src-tauri/src/lib.rs` | Tauri only: commands, file watcher, `noteki-file://`, saving on quit |
| `src-tauri/src/session.rs` | Everything the app does with a vault, without Tauri, returning events for the window. Tested |
| `src-tauri/src/document.rs` | `NoteDoc`: saving, Merge, Conflict, Deleted (SPEC › External changes) |
| `src-tauri/src/index.rs`, `markdown.rs` | In-memory corpus; search, tags, backlinks; the Markdown scanner they use |
| `vault.rs`, `wikilink.rs`, `frontmatter.rs`, `merge.rs`, `embeds.rs` | Vault I/O, Obsidian link resolution, frontmatter, three-way merge |

`document`, `index`, `markdown` and the updated `wikilink`/`frontmatter` come from the GPUI prototype (branch `gpui-prototype`), where they were fuzzed and mutation-tested.

## The editor protocol, and the race it now handles

The editor reports its text at most every 250 ms with a growing `revision`. It refuses a `reload` whose revision is not its last report, because the user typed after it. Before, the backend had already saved the merged text and forgot about it. The next save of the user's text then overwrote the other app's change. Now `app.ts` tells the backend (`reload_rejected`). The backend merges the disk change into the newest text again (or reports a Conflict), sends it, and saves it.
