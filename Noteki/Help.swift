import SwiftUI

/// The Help menu: Noteki's shortcuts in a window of their own, and its page, release notes and
/// issues on GitHub. In place of the system's search field and its "Noteki Help", which has no book.
struct HelpCommands: Commands {
    @Environment(\.openWindow) private var openWindow

    private static let repository = URL(string: "https://github.com/mameli/Noteki")!

    var body: some Commands {
        CommandGroup(replacing: .help) {
            Button("Keyboard Shortcuts") { openWindow(id: ShortcutsView.windowID) }
                .keyboardShortcut("/", modifiers: [.command, .shift])
            Divider()
            Link("Noteki on GitHub", destination: Self.repository)
            Link("Release Notes", destination: Self.repository.appending(path: "releases"))
            Link("Report an Issue", destination: Self.repository.appending(path: "issues/new"))
        }
    }
}

/// Every shortcut of the menus, by menu, to read at a glance.
struct ShortcutsView: View {
    static let windowID = "shortcuts"

    private struct Group: Identifiable {
        let name: String
        let shortcuts: [(String, String)]
        var id: String { name }
    }

    private let groups = [
        Group(name: "Notes", shortcuts: [
            ("New Note", "⌘N"), ("Today's Note", "⇧⌘D"), ("Quick Switcher", "⌘O"),
            ("Close Tab", "⌘W"), ("New Window", "⇧⌘N"), ("Open Vault", "⇧⌘O"),
        ]),
        Group(name: "Moving around", shortcuts: [
            ("Next Tab / Previous Tab", "⌃⇥ / ⌃⇧⇥"), ("Back / Forward", "⌥⌘← / ⌥⌘→"),
            ("Follow a link / in a new Tab", "⌘-click / ⌥⌘-click"),
            ("Files, Tags, Search", "⌘1, ⌘2, ⌘3"), ("Outline, Backlinks", "⌥⌘1, ⌥⌘2"),
            ("Left / Right Sidebar", "⌘0 / ⌥⌘0"),
        ]),
        Group(name: "Finding", shortcuts: [
            ("Find in the Note", "⌘F"), ("Find Next / Previous", "⌘G / ⇧⌘G"), ("Search in the Vault", "⇧⌘F"),
        ]),
        Group(name: "Format", shortcuts: [
            ("Bold, Italic", "⌘B, ⌘I"), ("Strikethrough", "⇧⌘X"), ("Inline Code", "⌃`"),
            ("Link", "⌘K"), ("Comment", "⌘/"), ("Task", "⌘↩"), ("Heading 1–6", "⌘1–⌘6 in the text"),
            ("Indent / Outdent", "⌘] / ⌘["),
        ]),
        Group(name: "View", shortcuts: [
            ("Source Mode", "⌘E"), ("Text Bigger / Smaller", "⌘+ / ⌘−"), ("Move to Trash, in Files", "⌘⌫"),
        ]),
    ]

    var body: some View {
        Form {
            ForEach(groups) { group in
                Section(group.name) {
                    ForEach(group.shortcuts, id: \.0) { name, keys in
                        LabeledContent(name) {
                            Text(keys).monospaced().foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .formStyle(.grouped)
        .frame(width: 440, height: 560)
    }
}
