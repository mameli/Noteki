import SwiftUI

/// The Help menu: Noteki's shortcuts, in the Command palette, which lists every command with its
/// shortcut, and its page, release notes and issues on GitHub. In place of the system's search
/// field and its "Noteki Help", which has no book.
struct HelpCommands: Commands {
    @FocusedValue(\.vaultWindow) private var vaultWindow
    @FocusedValue(\.noteWindow) private var sceneNoteWindow
    private var noteWindow: AppModel? { sceneNoteWindow ?? LooseFileFocus.shared.noteWindow }

    private static let repository = URL(string: "https://github.com/mameli/Noteki")!

    var body: some Commands {
        CommandGroup(replacing: .help) {
            Button("Keyboard Shortcuts") { (vaultWindow ?? noteWindow)?.showPalette(in: NSApp.keyWindow) }
                .disabled(vaultWindow ?? noteWindow == nil)
            Divider()
            Link("Noteki on GitHub", destination: Self.repository)
            Link("Release Notes", destination: Self.repository.appending(path: "releases"))
            Link("Report an Issue", destination: Self.repository.appending(path: "issues/new"))
        }
    }
}
