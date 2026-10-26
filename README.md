<p align="center">
  <img src="Noteki/Assets.xcassets/AppIcon.appiconset/icon_128x128@2x.png" alt="Noteki icon" width="128">
</p>

<h1 align="center">Noteki</h1>

<p align="center">macOS 15+ · Apple Silicon</p>

A light native Mac app to read and write the Markdown vault you keep with **Obsidian**, a fraction of its weight. It works on the same files, side by side with Obsidian, and keeps the parts used every day.

- **Live preview**: the Markdown syntax hides and shows again where the cursor is; tables, callouts, task lists, code blocks coloured in their language, images in the text.
- **Frontmatter** shown as a Properties table, with Tags as chips.
- **Wikilinks** resolved as Obsidian does, completed as you type, with **Backlinks** and an Outline in the sidebar.
- **Tags** pane, **Search** across the vault, a Quick switcher, the Daily note.
- **External changes** merged while you type: written for the Meeting notes [Takku](https://github.com/mameli/Takku) writes during a call.

## Build

Requires Xcode with Swift 6. The editor (CodeMirror 6, in `Editor/`) is committed already built in `Noteki/Resources/editor.html`, so the app builds without Node.

```sh
xcodebuild -project Noteki.xcodeproj -scheme Noteki -configuration Release -derivedDataPath build/Release build
```

To sign with your Developer ID, copy `Config/Local.xcconfig.example` to `Config/Local.xcconfig` and set your team. After changing the editor's sources, run `npm install && npm run build` in `Editor/`; `npm test` runs its tests.

## Docs

The specification is in [docs/SPEC.md](docs/SPEC.md), the vocabulary in [CONTEXT.md](CONTEXT.md) and the decisions in [docs/adr](docs/adr/). Code borrowed from other projects is listed in [THIRD_PARTY.md](THIRD_PARTY.md).

## License

[MIT](LICENSE)
