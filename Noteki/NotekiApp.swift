import NotekiCore
import SwiftUI
import UniformTypeIdentifiers

@main
struct NotekiApp: App {
    @NSApplicationDelegateAdaptor private var appDelegate: AppDelegate
    private var model: AppModel { appDelegate.model }

    var body: some Scene {
        // The Vault's window. Loose files open in windows of their own (AppDelegate); Tabs and
        // more windows of the Vault are ticket 25.
        Window("Noteki", id: "main") {
            ContentView(
                model: model,
                registerShowMainWindow: { appDelegate.showMainWindow = $0 },
                shouldAskForVault: { !appDelegate.hasLooseFiles }
            )
        }
        .defaultSize(width: 1100, height: 750)
        .commands {
            CommandGroup(after: .newItem) {
                Button("Quick Switcher…") {
                    appDelegate.showMainWindow()
                    model.showSwitcher(in: NSApp.keyWindow)
                }
                .keyboardShortcut("o")
                .disabled(!model.canShowSwitcher)
                Button("Open Vault…") { model.chooseVault() }
                    .keyboardShortcut("o", modifiers: [.command, .shift])
            }
            EditCommands(model: model)
            ZoomCommands()
        }
    }
}

/// Owns the models so that quitting can wait for the last edits to be saved, and receives the
/// files and folders opened from the Finder or another app (Takku).
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    let model: AppModel
    /// Reopens the main window, set by it when it first appears.
    var showMainWindow: () -> Void = {}
    private var isQuitting = false
    /// Loose files, each in its window.
    private var looseFileWindows: [NSWindow: AppModel] = [:]
    /// Windows saving their Loose file before closing: a second close waits for the first.
    private var closing: Set<NSWindow> = []

    var hasLooseFiles: Bool { !looseFileWindows.isEmpty }

    override init() {
        EditorController.registerSpellingDefaults()
        model = AppModel()
        super.init()
    }

    func application(_ application: NSApplication, open urls: [URL]) {
        for url in urls { open(url) }
    }

    /// A Note or Attachment of the Vault is shown from Files, a folder of the Vault is selected
    /// there; another folder may become the Vault; a text file from elsewhere opens alone.
    private func open(_ url: URL) {
        switch model.vault?.place(url) ?? .outside {
        case .note(let note):
            bringMainWindow()
            model.reveal(note.path)
        case .attachment(let attachment):
            bringMainWindow()
            model.reveal(attachment.path)
        case .folder(let path):
            bringMainWindow()
            if !path.isEmpty { model.reveal(path) }
        case .excluded:
            // The Vault's, though not listed: never a second editor on it.
            bringMainWindow()
        case .outside:
            if (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true {
                askToOpenAsVault(url)
            } else if UTType(filenameExtension: url.pathExtension)?.conforms(to: .text) == true {
                openLooseFile(url)
            }
        }
    }

    private func bringMainWindow() {
        showMainWindow()
        NSApp.activate()
    }

    private func askToOpenAsVault(_ folder: URL) {
        let alert = NSAlert()
        alert.messageText = "Open “\(folder.lastPathComponent)” as your Vault?"
        alert.informativeText = model.vault.map { "Noteki now shows “\($0.root.lastPathComponent)”." } ?? ""
        alert.addButton(withTitle: "Open as Vault")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        bringMainWindow()
        model.open(folder)
    }

    private func openLooseFile(_ file: URL) {
        let target = file.standardizedFileURL
        if let window = looseFileWindows.first(where: { $0.value.looseFileNow?.standardizedFileURL == target })?.key {
            window.makeKeyAndOrderFront(nil)
            return
        }
        let looseFile = AppModel(looseFile: file)
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 760, height: 640),
            styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false
        )
        window.isReleasedWhenClosed = false
        window.contentViewController = NSHostingController(rootView: LooseFileView(model: looseFile, file: file))
        window.delegate = self
        window.cascadeTopLeft(from: NSApp.keyWindow?.frame.origin ?? .zero)
        window.makeKeyAndOrderFront(nil)
        NSApp.activate()
        looseFileWindows[window] = looseFile
    }

    /// A Loose file's window saves it first, as quitting does; `close()` then skips this.
    func windowShouldClose(_ window: NSWindow) -> Bool {
        guard let looseFile = looseFileWindows[window] else { return true }
        guard closing.insert(window).inserted else { return false }
        Task {
            if await looseFile.prepareToQuit() { window.close() }
            closing.remove(window)
        }
        return false
    }

    func windowWillClose(_ notification: Notification) {
        guard let window = notification.object as? NSWindow else { return }
        looseFileWindows[window] = nil
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        // A second ⌘Q while the first one saves waits for it rather than asking twice.
        guard !isQuitting else { return .terminateLater }
        isQuitting = true
        Task {
            var canQuit = true
            for each in [model] + looseFileWindows.values where canQuit {
                canQuit = await each.prepareToQuit()
            }
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
