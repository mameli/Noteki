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
            ZoomCommands()
        }
    }
}

/// Owns the model so that quitting can wait for the last edits to be saved.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    let model = AppModel()
    private var isQuitting = false

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
