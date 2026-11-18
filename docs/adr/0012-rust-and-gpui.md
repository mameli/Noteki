# Noteki is written in Rust and drawn by GPUI, with its own editor element

Up to 0.3 Noteki was SwiftUI around CodeMirror 6 in a WKWebView ([ADR 0001](0001-codemirror-in-wkwebview.md)). It worked, but the web view carried most of the weight and most of the bugs: WebKit's processes were 50 to 90 MB of the memory budget, plus about 100 MB while the editor had the keyboard; every editor feature lived twice, as TypeScript and as the Swift bridge to it; and focus, find, undo and key handling had to be wrestled from WebKit (the code taken from MarkEdit, ADR 0007).

From 0.4 Noteki is one Rust program on [GPUI](https://www.gpui.rs) (Zed's UI framework) with GPUI Kit's components. A prototype first answered whether GPUI could replace SwiftUI + CodeMirror ([docs/gpui-evaluation.md](../gpui-evaluation.md)): yes for the shell, the Vault, saving, merging, links and search; not with GPUI Kit's `Editor`, which can colour Markdown but cannot hide marks or draw tables, images and formulas in the text. So Noteki has its own editor element (`src/editor/`): it shapes and paints each line on its own, with the display map that hides marks off the cursor's line, tables drawn as grids, images, formulas typeset by RaTeX and Mermaid diagrams drawn as SVG, all in the same window and process.

- **One language, one process**: the rules (`src/core`, no GPUI) are tested as plain Rust, and the whole app headlessly with GPUI's `TestAppContext`; no bridge, no editor bundle to rebuild ([ADR 0004](0004-committed-editor-bundle.md) goes), no web view per window ([ADR 0003](0003-one-web-view-per-window.md) goes).
- **Same app to the user**: the same bundle id (`app.noteki.noteki`), the same Sparkle key and feed (ADR 0011), so 0.3 updates itself to 0.4; the first launch takes 0.3's preferences from its defaults (`src/app/swift_prefs.rs`).
- **Linux for free**: GPUI runs on Wayland and X11, so the same code builds a Linux app (`scripts/package-linux.sh`), with the shortcuts its apps use.

## Consequences

- The Swift code (`Noteki/`, `NotekiCore/`, `Editor/`, the Xcode project) is removed from the repository; it stays in the history and in the 0.3.1 release, the last version with a web view.
- Everything CodeMirror and WebKit gave for free is Noteki's own to write and test: text input and IME, selection, undo, wrapping, spelling (the Mac's NSSpellChecker), accessibility. WebKit is still the system's way to page HTML: *Export as PDF…* and *Print…* lay the exported page out in a WKWebView never shown, only while printing (`src/app/print.rs`).
- The SPEC's budget was set for a web view: the app is 26 MB on disk with Sparkle, over its 20 MB, and memory has to be measured again without WebKit.
