import SwiftUI

@main
struct NotekiApp: App {
    @NSApplicationDelegateAdaptor private var appDelegate: AppDelegate
    private var model: AppModel { appDelegate.model }

    var body: some Scene {
        // One window until Noteki has its own Tabs and windows (ticket 25).
        Window("Noteki", id: "main") {
            ContentView(model: model)
        }
        .defaultSize(width: 1100, height: 750)
        .commands {
            CommandGroup(after: .newItem) {
                Button("Open Vault…") { model.chooseVault() }
                    .keyboardShortcut("o", modifiers: [.command, .shift])
            }
            EditCommands(model: model)
            ZoomCommands()
        }
    }
}

/// Owns the model so that quitting can wait for the last edits to be saved.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    let model: AppModel
    private var isQuitting = false

    override init() {
        EditorController.registerSpellingDefaults()
        model = AppModel()
        super.init()
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        // A second ⌘Q while the first one saves waits for it rather than asking twice.
        guard !isQuitting else { return .terminateLater }
        isQuitting = true
        Task {
            let canQuit = await model.prepareToQuit()
            isQuitting = false
            sender.reply(toApplicationShouldTerminate: canQuit)
        }
        return .terminateLater
    }
}

extension FocusedValues {
    /// The window's editor while a Note is in front of it, nil otherwise.
    @Entry var noteEditor: EditorController?
}

/// Find and spelling, after Cut, Copy and Paste. Find follows the window: off while an
/// Attachment is in front.
private struct EditCommands: Commands {
    let model: AppModel
    @FocusedValue(\.noteEditor) private var editor

    var body: some Commands {
        CommandGroup(after: .pasteboard) {
            Divider()
            Menu("Find") {
                Button("Find…") { editor?.find(.open) }
                    .keyboardShortcut("f")
                Button("Find Next") { editor?.find(.next) }
                    .keyboardShortcut("g")
                Button("Find Previous") { editor?.find(.previous) }
                    .keyboardShortcut("g", modifiers: [.command, .shift])
            }
            .disabled(editor == nil)
            SpellingMenu(editor: model.editor.webView)
        }
    }
}

/// The editor's spelling settings. WebKit keeps them in the app's preferences, which is where
/// the checkmarks read them; the toggles go to the editor even when the sidebar has the focus.
private struct SpellingMenu: View {
    let editor: EditorWebView
    @AppStorage("WebContinuousSpellCheckingEnabled") private var checksSpelling = true
    @AppStorage("WebAutomaticSpellingCorrectionEnabled") private var correctsSpelling = true

    var body: some View {
        Menu("Spelling and Grammar") {
            Toggle("Check Spelling While Typing", isOn: Binding(
                get: { checksSpelling },
                set: { isOn in
                    toggle(#selector(NSTextView.toggleContinuousSpellChecking(_:)))
                    checksSpelling = isOn
                }
            ))
            Toggle("Correct Spelling Automatically", isOn: Binding(
                get: { correctsSpelling },
                set: { isOn in
                    toggle(#selector(NSTextView.toggleAutomaticSpellingCorrection(_:)))
                    correctsSpelling = isOn
                }
            ))
        }
    }

    private func toggle(_ action: Selector) {
        NSApp.sendAction(action, to: editor, from: nil)
    }
}

/// Zoom for what is shown, whatever has the focus in the window: the sidebar usually does.
struct ZoomActions {
    let zoomIn: () -> Void
    let zoomOut: () -> Void
}

extension FocusedValues {
    @Entry var zoom: ZoomActions?
}

private struct ZoomCommands: Commands {
    @FocusedValue(\.zoom) private var zoom

    var body: some Commands {
        CommandGroup(after: .toolbar) {
            Button("Zoom In") { zoom?.zoomIn() }
                .keyboardShortcut("+")
                .disabled(zoom == nil)
            Button("Zoom Out") { zoom?.zoomOut() }
                .keyboardShortcut("-")
                .disabled(zoom == nil)
        }
    }
}
