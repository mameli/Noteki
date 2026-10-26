import NotekiCore
import SwiftUI
import UniformTypeIdentifiers

@main
struct NotekiApp: App {
    @NSApplicationDelegateAdaptor private var appDelegate: AppDelegate
    private var model: AppModel { appDelegate.model }

    var body: some Scene {
        // The Vault's first window, and the ones ⌘⇧N opens: each its own model and web view (ADR
        // 0010). Loose files open in windows of their own (AppDelegate).
        Window("Noteki", id: "main") {
            ContentView(
                model: model,
                registerShowMainWindow: { appDelegate.showMainWindow = $0 },
                shouldAskForVault: { !appDelegate.hasLooseFiles },
                windows: appDelegate.windows
            )
        }
        .defaultSize(width: 1100, height: 750)
        .commands {
            CommandGroup(replacing: .newItem) {
                VaultCommands(main: model, windows: appDelegate.windows, showMainWindow: { appDelegate.showMainWindow() })
            }
            CommandGroup(after: .sidebar) {
                SidebarCommands(main: model)
            }
            CommandMenu("Go") {
                GoCommands()
            }
            ViewCommands()
            EditCommands(main: model, showMainWindow: { appDelegate.showMainWindow() })
            ZoomCommands()
        }
        WindowGroup("Noteki", id: "vault", for: UUID.self) { $id in
            if let id, let model = appDelegate.windows.models[id] {
                ContentView(model: model, registerShowMainWindow: { _ in }, shouldAskForVault: { false }, windows: appDelegate.windows, windowID: id)
            }
        }
        .defaultSize(width: 1100, height: 750)
        .restorationBehavior(.disabled)
    }
}

/// The Vault's windows after the first, ⌘⇧N, and their Tabs kept for the next launch: the
/// first window's Tabs, then theirs in the order they were opened (ADR 0010).
@MainActor
@Observable
final class VaultWindows {
    private(set) var models: [UUID: AppModel] = [:]
    @ObservationIgnored private var order: [UUID] = []
    @ObservationIgnored private let main: AppModel
    /// Grows when the Vault changes: the other windows close, they showed the Vault left.
    private(set) var vaultChanges = 0
    /// While the app quits: the windows that close are still kept for the next launch.
    @ObservationIgnored var isQuitting = false
    /// The other windows to open, once the first is on screen: at launch, or for another Vault.
    @ObservationIgnored private(set) var toRestore: [Tabs] = []
    /// Opens a window again, given by the first window once it is on screen.
    @ObservationIgnored var reopen: (UUID) -> Void = { _ in }
    private static let key = "windows"

    init(main: AppModel) {
        self.main = main
        restoreSaved()
        main.onTabsChanged = { [weak self] _ in self?.remember() }
        main.onVaultChanged = { [weak self] in self?.vaultChanged() }
        shareFileChanges(of: main)
    }

    /// A file renamed, moved or put in the Trash in one window: the others follow.
    private func shareFileChanges(of model: AppModel) {
        model.onFileChange = { [weak self, weak model] change in
            for window in self?.all ?? [] where window !== model { window.follow(change) }
        }
    }

    var all: [AppModel] { [main] + order.compactMap { models[$0] } }

    /// A new window of the Vault, showing `tabs`; its id for `openWindow`.
    func add(showing tabs: Tabs = Tabs()) -> UUID? {
        guard let vault = main.vault else { return nil }
        let model = AppModel(vault: vault.root)
        model.restore(tabs)
        let id = UUID()
        models[id] = model
        order.append(id)
        model.onTabsChanged = { [weak self] _ in self?.remember() }
        shareFileChanges(of: model)
        remember()
        return id
    }

    /// The windows to open, taken once.
    func takeToRestore() -> [Tabs] {
        defer { toRestore = [] }
        return toRestore
    }

    /// A window closed: its Notes saved, and its Tabs no longer kept, unless the app is quitting.
    /// SwiftUI closes it before asking: a user who chooses to stay with edits not saved gets it
    /// back as it was.
    func closed(_ id: UUID) {
        guard let model = models[id] else { return }
        Task {
            guard await model.prepareToQuit() else { return reopen(id) }
            guard !isQuitting else { return }
            models[id] = nil
            order.removeAll { $0 == id }
            remember()
        }
    }

    /// The first window's Tabs and the other windows kept for the Vault now open.
    private func restoreSaved() {
        guard let path = main.vault?.root.path(percentEncoded: false),
              let saved = (UserDefaults.standard.dictionary(forKey: Self.key)?[path] as? [Data])?
                .compactMap({ try? JSONDecoder().decode(Tabs.self, from: $0) }),
              let first = saved.first
        else { return }
        // First: the first window's Tabs coming back are remembered with the others.
        toRestore = Array(saved.dropFirst())
        main.restore(first)
    }

    private func vaultChanged() {
        vaultChanges += 1
        restoreSaved()
    }

    /// The Tabs of the windows of the Vault now open, those still to open included; not those of
    /// windows on their way out with the Vault left.
    private func remember() {
        guard let vault = main.vault else { return }
        let path = vault.root.path(percentEncoded: false)
        let windows = all.filter { $0.vault?.root == vault.root }.map(\.tabs) + toRestore
        var all = UserDefaults.standard.dictionary(forKey: Self.key) ?? [:]
        all[path] = windows.compactMap { try? JSONEncoder().encode($0) }
        UserDefaults.standard.set(all, forKey: Self.key)
    }
}

/// Owns the models so that quitting can wait for the last edits to be saved, and receives the
/// files and folders opened from the Finder or another app (Takku).
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    let model: AppModel
    let windows: VaultWindows
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
        windows = VaultWindows(main: model)
        super.init()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        // ⌃Tab and ⌃⇧Tab are the Go menu's, wherever the keyboard is: the editor and the
        // sidebar's lists would take them to move the keyboard to another view.
        NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            guard event.keyCode == 48, modifiers == .control || modifiers == [.control, .shift] else { return event }
            return NSApp.mainMenu?.performKeyEquivalent(with: event) == true ? nil : event
        }
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
            model.revealFromOutside(note.path)
        case .attachment(let attachment):
            bringMainWindow()
            model.revealFromOutside(attachment.path)
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
        windows.isQuitting = true
        Task {
            var canQuit = true
            for each in windows.all + looseFileWindows.values where canQuit {
                canQuit = await each.prepareToQuit()
            }
            isQuitting = false
            windows.isQuitting = canQuit
            sender.reply(toApplicationShouldTerminate: canQuit)
        }
        return .terminateLater
    }
}

extension FocusedValues {
    /// The window's editor while a Note is in front of it, nil otherwise.
    @Entry var noteEditor: EditorController?
    /// The window's model while a Note is in front of it: Source mode is its own.
    @Entry var noteWindow: AppModel?
    /// The model of the Vault's window in front, whatever it shows: its Tabs, its sidebars.
    @Entry var vaultWindow: AppModel?
}

/// After New: the Quick switcher and the Tabs of the Vault's window in front (or the first
/// window's, from a Loose file's), a new window, another Vault.
private struct VaultCommands: View {
    let main: AppModel
    let windows: VaultWindows
    let showMainWindow: () -> Void
    @FocusedValue(\.vaultWindow) private var window
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Button("New Note") {
            if window == nil { showMainWindow() }
            (window ?? main).newNote()
        }
        .keyboardShortcut("n")
        .disabled(main.vault == nil)
        Button("New Window") {
            if let id = windows.add() { openWindow(id: "vault", value: id) }
        }
        .keyboardShortcut("n", modifiers: [.command, .shift])
        .disabled(main.vault == nil)
        Button("Quick Switcher…") {
            if window == nil { showMainWindow() }
            (window ?? main).showSwitcher(in: NSApp.keyWindow)
        }
        .keyboardShortcut("o")
        .disabled(!(window ?? main).canShowSwitcher)
        Button("Open Vault…") { main.chooseVault() }
            .keyboardShortcut("o", modifiers: [.command, .shift])
        Divider()
        // Before the window's own Close: with no Tab open, ⌘W closes the window.
        Button("Close Tab") {
            if let window, let tab = window.tabs.front { window.closeTab(tab.id) } else { NSApp.keyWindow?.performClose(nil) }
        }
        .keyboardShortcut("w")
    }
}

private struct SidebarCommands: View {
    let main: AppModel
    @FocusedValue(\.vaultWindow) private var window

    var body: some View {
        let model = window ?? main
        Button(model.isShowingSidebar ? "Hide Right Sidebar" : "Show Right Sidebar") {
            model.isShowingSidebar.toggle()
        }
        .keyboardShortcut("0", modifiers: [.command, .option])
    }
}

/// Back and forward in the Tab in front, and from one Tab to the next.
private struct GoCommands: View {
    @FocusedValue(\.vaultWindow) private var window

    var body: some View {
        Button("Back") { window?.goBack() }
            .keyboardShortcut(.leftArrow, modifiers: [.command, .option])
            .disabled(window?.canGoBack != true)
        Button("Forward") { window?.goForward() }
            .keyboardShortcut(.rightArrow, modifiers: [.command, .option])
            .disabled(window?.canGoForward != true)
        Divider()
        Button("Show Previous Tab") { window?.selectPreviousTab() }
            .keyboardShortcut(.tab, modifiers: [.control, .shift])
            .disabled((window?.tabs.all.count ?? 0) < 2)
        Button("Show Next Tab") { window?.selectNextTab() }
            .keyboardShortcut(.tab, modifiers: .control)
            .disabled((window?.tabs.all.count ?? 0) < 2)
    }
}

/// Find and spelling, after Cut, Copy and Paste. Find follows the window: off while an
/// Attachment is in front. Search in Vault is the window's in front, or the first window's.
private struct EditCommands: Commands {
    let main: AppModel
    /// Search is the Vault's: from a Loose file's window, ⌘⇧F brings the Vault's window.
    let showMainWindow: () -> Void
    @FocusedValue(\.noteEditor) private var editor
    @FocusedValue(\.vaultWindow) private var window

    var body: some Commands {
        CommandGroup(after: .pasteboard) {
            Divider()
            Menu("Find") {
                Button("Find…") { editor?.find(.open) }
                    .keyboardShortcut("f")
                    .disabled(editor == nil)
                Button("Find Next") { editor?.find(.next) }
                    .keyboardShortcut("g")
                    .disabled(editor == nil)
                Button("Find Previous") { editor?.find(.previous) }
                    .keyboardShortcut("g", modifiers: [.command, .shift])
                    .disabled(editor == nil)
                Divider()
                Button("Search in Vault…") {
                    if window == nil { showMainWindow() }
                    (window ?? main).showSearch()
                }
                .keyboardShortcut("f", modifiers: [.command, .shift])
                .disabled(main.vault == nil)
            }
            SpellingMenu(editor: (window ?? main).editor.webView)
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

/// Source mode, for the Note in front of the window.
private struct ViewCommands: Commands {
    @FocusedValue(\.noteWindow) private var window

    var body: some Commands {
        CommandGroup(before: .toolbar) {
            Toggle("Source Mode", isOn: Binding(
                get: { window?.isSourceMode ?? false },
                set: { window?.prefersSourceMode = $0 }
            ))
            .keyboardShortcut("e")
            // A Note too large for Live preview stays in Source mode.
            .disabled(window == nil || window?.isSourceModeForced == true)
            Divider()
        }
    }
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
