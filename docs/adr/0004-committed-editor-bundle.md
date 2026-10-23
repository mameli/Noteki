# The editor's JavaScript is built into one HTML file that is committed

The editor's sources are TypeScript in `Editor/`, built with Node into a single `editor.html` with everything inlined, as MarkEdit does: no requests to load it, and Swift fills in the configuration before loading it, so the first Note appears without a round trip. Unlike MarkEdit, the built file is committed in the app's resources. Building the app then needs only Xcode, and Node is needed only to change the editor, which happens far less often than changing the Swift side.

## Consequences

- The bundle carries a hash of the sources in `Editor/`, and an Xcode build phase fails when it does not match them, so a forgotten rebuild is caught. A hash rather than modification times, which change with every clone.
- Diffs of the built file are noise: review the sources, not the bundle.
- The editor's tests (`Editor/test/`) are not in the bundle, nor in its hash: changing a test needs no rebuild.
