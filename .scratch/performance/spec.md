# Performance: memory at rest and time per key

What 0.4.1 measured and changed, and what is left. Budgets are in docs/SPEC.md › Budget; how to measure is below.

## How it is measured

- **Memory**: `footprint -p <pid>` on the release build, the window in front at 1280×805, 13 s after the last input (the caret has stopped blinking), the Mac otherwise idle: a pointer over the window or another app drawing changes the graphics figures. A copy of `docs/demo-vault` and `NOTEKI_SETTINGS`/`NOTEKI_VAULT` keep the user's own app and preferences out of it.
- **Time per key**: `cargo test --release --test keystroke_timing -- --ignored --nocapture` (a 112 KB Note, 75 Headings); `NOTEKI_KT_PARAGRAPHS=1` writes it in paragraphs, `NOTEKI_KT_KEYS=1500` types long enough to profile it with `sample` (symbols kept: `CARGO_PROFILE_RELEASE_STRIP=false`). Since issue 01 a key is one frame drawn as the app draws it; until then the test drew three frames with `refresh` per key, which ignores cached views, so its figures before that are not comparable.

## Where 0.4.1 left it

| | 0.4.0 | 0.4.1 |
|---|---:|---:|
| A Note open, at rest | 165–188 MB | 68–76 MB (Launch plan 110) |
| A key, the 112 KB Note | 15.9 ms | 6.9 ms |
| The same Note in paragraphs | 9.0 ms | 4.6 ms |
| An arrow key | 7.9 ms | 3.6 ms |

What a key still costs, from the last profile: GPUI laying out the whole window again at every frame (the sidebars, the title and status bars, their buttons), then Live preview's block styles and the editor's own layout. Issue 01.

## After issue 01 (the parts around the editor cached)

The same test, a key and its one frame (both columns measured this way):

| | Before | After |
|---|---:|---:|
| A key, the 112 KB Note | 4.13 ms | 3.3 ms |
| The same Note in paragraphs | 2.09 ms | 1.24 ms |
| An arrow key | 1.30 ms | 0.49 ms |
| A frame of the whole window (`refresh`) | 0.97 ms | 1.0 ms |

