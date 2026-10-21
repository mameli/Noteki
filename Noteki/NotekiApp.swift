import SwiftUI

@main
struct NotekiApp: App {
    @State private var model = AppModel()

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
