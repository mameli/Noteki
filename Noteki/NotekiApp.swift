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
        }
    }
}
