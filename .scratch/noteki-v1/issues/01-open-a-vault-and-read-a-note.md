# 01: Open a Vault and read a Note

**What to build:** At first launch Noteki asks for the Vault folder and remembers it for the next launches. The Files pane shows the folders and Notes of the Vault; clicking a Note shows its text in the CodeMirror editor, read only, in Source mode with Markdown syntax colouring. This is the tracer bullet through every layer: the NotekiCore Vault module, the app window, the editor bundle loaded in a WKWebView, and the first bridge message.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [x] Choosing a folder at first launch opens it as the Vault; relaunching reopens it without asking
- [x] *File → Open Vault…* switches to another folder
- [x] The Files pane lists folders and `.md` Notes as a tree, folders first, names sorted as the Finder does
- [x] Clicking a Note shows its full text in the editor, read only, with Markdown syntax colouring
- [x] The editor bundle is one HTML file committed in the app's resources; a build phase fails when it is out of date with the editor's sources (a hash of them)
- [x] The Vault module is tested at Seam 1 on a temporary folder: the tree it lists and the text it loads
- [x] Noteki writes nothing inside the Vault

## Comments

Done. Verified in the app on a demo Vault: the remembered Vault reopens, a Vault that no longer exists asks again, folders come first in Finder order, non-Markdown files are hidden, a Note shows read only with grey Frontmatter. Known and left to later tickets: `.obsidian` is still listed (02); the bridge contract test moves to 04; settings injected before load move to 29.
