# The editor's JavaScript is built into one HTML file that is committed

The editor's sources are TypeScript in `Editor/`, built with Node into a single `index.html` with everything inlined, as MarkEdit does: no requests to load it, and Swift fills in the configuration before loading it, so the first Note appears without a round trip. Unlike MarkEdit, the built file is committed in the app's resources. Building the app then needs only Xcode, and Node is needed only to change the editor, which happens far less often than changing the Swift side.

## Consequences

- An Xcode build phase fails when the bundle is older than the sources in `Editor/`, so a forgotten rebuild is caught.
- Diffs of the built file are noise: review the sources, not the bundle.
