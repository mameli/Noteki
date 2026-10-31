import NotekiCore
import SwiftUI

/// A row of Format buttons over the Note, for whoever does not know the shortcuts yet: shown
/// only when chosen in Settings. Each does what its Format menu item does.
struct FormatBar: View {
    let model: AppModel

    var body: some View {
        HStack(spacing: 2) {
            // On the right, out of the way of the text's start.
            Spacer()
            Menu {
                ForEach(1...6, id: \.self) { level in
                    Button("Heading \(level)") { format(EditorCommand.Formatting(rawValue: "heading\(level)")!) }
                }
            } label: {
                Image(systemName: "textformat.size")
            }
            .menuIndicator(.hidden)
            .fixedSize()
            .help("Heading")
            button("bold", "Bold (⌘B)", .bold)
            button("italic", "Italic (⌘I)", .italic)
            button("strikethrough", "Strikethrough (⇧⌘X)", .strikethrough)
            button("text.quote", "Quote", .quote)
            button("chevron.left.forwardslash.chevron.right", "Inline Code (⌃`)", .code)
            button("link", "Link (⌘K)", .link)
            Divider().frame(height: 16).padding(.horizontal, 4)
            button("list.number", "Numbered List", .numberedList)
            button("list.bullet", "Bulleted List", .bulletList)
            button("checklist", "Task (⌘↩)", .task)
            Divider().frame(height: 16).padding(.horizontal, 4)
            Button { model.attachFiles() } label: { Image(systemName: "paperclip") }
                .help("Attach Images…")
        }
        // Quiet: grey icons on the page's own background, no bar or line under it.
        .buttonStyle(.borderless)
        .foregroundStyle(.secondary)
        .imageScale(.small)
        .padding(.horizontal, 10)
        .padding(.vertical, 3)
        .background(.background)
    }

    private func button(_ symbol: String, _ help: String, _ formatting: EditorCommand.Formatting) -> some View {
        Button { format(formatting) } label: {
            Image(systemName: symbol).frame(width: 22, height: 20)
        }
        .help(help)
        .accessibilityLabel(help)
    }

    private func format(_ formatting: EditorCommand.Formatting) {
        model.editor.format(formatting)
        model.editor.focus()
    }
}
