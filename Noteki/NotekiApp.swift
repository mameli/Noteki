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
        // A file opened from the Finder or Takku is the AppDelegate's: left to SwiftUI, the
        // window is presented again, with the animation of a new one.
        .handlesExternalEvents(matching: [])
        .commands {
            CommandGroup(replacing: .newItem) {
                VaultCommands(main: model, windows: appDelegate.windows, showMainWindow: { appDelegate.showMainWindow() })
            }
            // ⌘0 and ⌥⌘0 for the two sidebars, as in Xcode; in place of the system's ⌃⌘S.
            CommandGroup(replacing: .sidebar) {
                SidebarCommands(main: model)
            }
            CommandMenu("Go") {
                GoCommands()
            }
            ViewCommands()
            CommandMenu("Format") {
                FormatCommands()
            }
            EditCommands(main: model, showMainWindow: { appDelegate.showMainWindow() })
            ZoomCommands()
            HelpCommands()
            UpdateCommands()
            ExportCommands()
        }
        WindowGroup("Noteki", id: "vault", for: UUID.self) { $id in
            if let id, let model = appDelegate.windows.models[id] {
                ContentView(model: model, registerShowMainWindow: { _ in }, shouldAskForVault: { false }, windows: appDelegate.windows, windowID: id)
            }
        }
        .defaultSize(width: 1100, height: 750)
        .restorationBehavior(.disabled)
        // A file opened from the Finder or Takku is the AppDelegate's: left to SwiftUI, a
        // WindowGroup answers it with a window of its own, a second view of the Vault.
        .handlesExternalEvents(matching: [])
        Settings {
            SettingsView(model: model)
        }
        // Nearly square at first; taller to see more without scrolling.
        .defaultSize(width: 500, height: 540)
        .windowResizability(.contentMinSize)
        .handlesExternalEvents(matching: [])
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
    var showMainWindow: () -> Void = { AppDelegate.openMainWindowFromMenu() }

    /// Before the main window has ever appeared (Noteki opened with a Loose file), it has not
    /// registered its way back: the Window menu's item for its scene opens it, as a click would.
    private static func openMainWindowFromMenu() {
        guard let menu = NSApp.windowsMenu,
              let index = menu.items.firstIndex(where: { $0.title == "Noteki" && $0.action != #selector(NSWindow.makeKeyAndOrderFront(_:)) })
        else { return }
        menu.performActionForItem(at: index)
    }
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

    func applicationWillFinishLaunching(_ notification: Notification) {
        // Before the first window shows: it would flash in the Mac's appearance.
        AppSettings.shared.applyAppearance()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        UpdateChecker.shared.start()
        // ⌃Tab and ⌃⇧Tab are the Go menu's, wherever the keyboard is: the editor and the
        // sidebar's lists would take them to move the keyboard to another view.
        NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            // ⇧⌘] and ⇧⌘[ too, as in Safari and Xcode: the same items as ⌃Tab and ⌃⇧Tab, which
            // a menu item cannot have as a second shortcut.
            if modifiers == [.command, .shift], event.keyCode == 30 || event.keyCode == 33,
               let tab = NSEvent.keyEvent(
                   with: .keyDown, location: event.locationInWindow, modifierFlags: event.keyCode == 30 ? .control : [.control, .shift],
                   timestamp: event.timestamp, windowNumber: event.windowNumber, context: nil,
                   characters: "\t", charactersIgnoringModifiers: "\t", isARepeat: false, keyCode: 48
               ) {
                return NSApp.mainMenu?.performKeyEquivalent(with: tab) == true ? nil : event
            }
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
        // Known before it becomes key: the menus follow it from then (LooseFileFocus).
        looseFileWindows[window] = looseFile
        window.makeKeyAndOrderFront(nil)
        NSApp.activate()
    }

    func windowDidBecomeKey(_ notification: Notification) {
        if let window = notification.object as? NSWindow, let looseFile = looseFileWindows[window] {
            LooseFileFocus.shared.model = looseFile
        }
    }

    func windowDidResignKey(_ notification: Notification) {
        if let window = notification.object as? NSWindow, LooseFileFocus.shared.model === looseFileWindows[window] {
            LooseFileFocus.shared.model = nil
        }
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

/// The Loose file whose window is key. Its window is AppKit's, not a SwiftUI scene: the menus'
/// focused values never see it, and without this its Format, Find, Export and palette items stayed
/// disabled. The menus read it when no scene gives them a value.
@MainActor
@Observable
final class LooseFileFocus {
    static let shared = LooseFileFocus()
    var model: AppModel?

    /// As `noteWindow`: the model while a Note is in front of the window.
    var noteWindow: AppModel? { model?.isShowingNote == true ? model : nil }
    /// As `noteEditor`.
    var editor: EditorController? { noteWindow?.editor }
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
        Button("Today's Note") {
            if window == nil { showMainWindow() }
            (window ?? main).openTodaysNote()
        }
        // Not ⌥⌘D, as the SPEC first said: macOS takes it to hide the Dock.
        .keyboardShortcut("d", modifiers: [.command, .shift])
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
        // As in Obsidian and the browsers; the Note chosen in the Quick switcher opens in it.
        Button("New Tab…") {
            if window == nil { showMainWindow() }
            (window ?? main).showSwitcher(in: NSApp.keyWindow, inNewTab: true)
        }
        .keyboardShortcut("t")
        .disabled(!(window ?? main).canShowSwitcher)
        Button("Open Vault…") { main.chooseVault() }
            .keyboardShortcut("o", modifiers: [.command, .shift])
        // The Vault's folder; a folder of it from its menu in Files.
        Button("Open Vault in \(TerminalApp.name)") {
            if let root = (window ?? main).vault?.root { TerminalApp.open(root) }
        }
        // No shortcut: ⌥⌘T is the View menu's Hide Toolbar.
        .disabled((window ?? main).vault == nil)
        Divider()
        // Before the window's own Close: with no Tab open, ⌘W closes the window.
        Button("Close Tab") {
            if let window, let tab = window.tabs.front { window.closeTab(tab.id) } else { NSApp.keyWindow?.performClose(nil) }
        }
        .keyboardShortcut("w")
        Button("Reopen Closed Tab") { window?.reopenClosedTab() }
            .keyboardShortcut("t", modifiers: [.command, .shift])
            .disabled(window?.canReopenClosedTab != true)
    }
}

private struct SidebarCommands: View {
    let main: AppModel
    @FocusedValue(\.vaultWindow) private var window

    var body: some View {
        let model = window ?? main
        // The window's split view shows or hides Files, Search and Tags.
        Button("Toggle Left Sidebar") {
            NSApp.sendAction(#selector(NSSplitViewController.toggleSidebar(_:)), to: nil, from: nil)
        }
        // The Mac's own; ⌘0 is Actual Size, as everywhere.
        .keyboardShortcut("s", modifiers: [.command, .control])
        Button(model.isShowingSidebar ? "Hide Right Sidebar" : "Show Right Sidebar") {
            model.isShowingSidebar.toggle()
        }
        // Xcode's for its inspector. Not ⌃⌥⌘S: window managers take ⌃⌥⌘ chords before the app.
        .keyboardShortcut("0", modifiers: [.command, .option])
        Divider()
        // ⌃⌘: ⌘1–⌘6 are the Headings, as in Typora.
        Button("Files") { model.leftPane = .files }.keyboardShortcut("1", modifiers: [.command, .control])
        Button("Tags") { model.leftPane = .tags }.keyboardShortcut("2", modifiers: [.command, .control])
        Button("Search") { model.leftPane = .search }.keyboardShortcut("3", modifiers: [.command, .control])
        Button("Outline") { model.rightPane = .outline; model.isShowingSidebar = true }
            .keyboardShortcut("1", modifiers: [.command, .option])
        Button("Backlinks") { model.rightPane = .backlinks; model.isShowingSidebar = true }
            .keyboardShortcut("2", modifiers: [.command, .option])
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
    @FocusedValue(\.noteEditor) private var sceneEditor
    private var editor: EditorController? { sceneEditor ?? LooseFileFocus.shared.editor }
    @FocusedValue(\.vaultWindow) private var window

    var body: some Commands {
        CommandGroup(after: .pasteboard) {
            // Not in SwiftUI's Edit menu: the text alone, never turned into Markdown, sent to the
            // editor's web view (EditorWebView.pasteAsPlainText), as a text field gets it. ⇧⌘V as
            // in Obsidian (and Chrome, Slack, Notion), not the Mac's ⌥⇧⌘V.
            Button("Paste and Match Style") {
                NSApp.sendAction(#selector(EditorWebView.pasteAsPlainText(_:)), to: nil, from: nil)
            }
            .keyboardShortcut("v", modifiers: [.command, .shift])
            .disabled(editor == nil)
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
                Button("Replace…") { editor?.find(.replace) }
                    .keyboardShortcut("f", modifiers: [.command, .option])
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

/// The editor's spelling settings, the same as in Settings.
private struct SpellingMenu: View {
    let editor: EditorWebView

    var body: some View {
        Menu("Spelling and Grammar") {
            SpellingToggles(editor: editor, checks: "Check Spelling While Typing", corrects: "Correct Spelling Automatically")
        }
    }
}

/// WebKit's own spelling settings, one for the whole app: it keeps them in the app's preferences,
/// which is where the checkmarks read them, and a toggle sent to any editor's web view changes
/// them for all, at once. The Edit menu and Settings both show them.
struct SpellingToggles: View {
    /// The editor to toggle; any window's when nil (Settings).
    var editor: EditorWebView?
    let checks: String
    let corrects: String
    @AppStorage("WebContinuousSpellCheckingEnabled") private var checksSpelling = true
    @AppStorage("WebAutomaticSpellingCorrectionEnabled") private var correctsSpelling = true

    var body: some View {
        Toggle(checks, isOn: Binding(
            get: { checksSpelling },
            set: { isOn in
                toggle(#selector(NSTextView.toggleContinuousSpellChecking(_:)))
                checksSpelling = isOn
            }
        ))
        Toggle(corrects, isOn: Binding(
            get: { correctsSpelling },
            set: { isOn in
                toggle(#selector(NSTextView.toggleAutomaticSpellingCorrection(_:)))
                correctsSpelling = isOn
            }
        ))
    }

    private func toggle(_ action: Selector) {
        guard let webView = editor ?? NSApp.windows.lazy.compactMap({ $0.contentView?.firstEditorWebView }).first else { return }
        NSApp.sendAction(action, to: webView, from: nil)
    }
}

private extension NSView {
    var firstEditorWebView: EditorWebView? {
        if let webView = self as? EditorWebView { return webView }
        return subviews.lazy.compactMap(\.firstEditorWebView).first
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

/// Markdown written around the selection or on its lines, with Obsidian's shortcuts where it
/// has them, Typora's and Bear's otherwise.
private struct FormatCommands: View {
    @FocusedValue(\.noteEditor) private var sceneEditor
    private var editor: EditorController? { sceneEditor ?? LooseFileFocus.shared.editor }

    var body: some View {
        Group {
            item("Bold", .bold, "b")
            item("Italic", .italic, "i")
            item("Strikethrough", .strikethrough, "x", [.command, .shift])
            item("Inline Code", .code, "`", .control)
            item("Comment", .comment, "/")
            Divider()
            item("Link", .link, "k")
            Divider()
            Button("Quote") { editor?.format(.quote) }
            Button("Bulleted List") { editor?.format(.bulletList) }
            Button("Numbered List") { editor?.format(.numberedList) }
            item("Task", .task, .return)
            Divider()
            Menu("Heading") {
                ForEach(1...6, id: \.self) { level in
                    item("Heading \(level)", EditorCommand.Formatting(rawValue: "heading\(level)")!, KeyEquivalent(Character("\(level)")))
                }
            }
            Divider()
            Button("Indent") { editor?.indent(outdent: false) }
                .keyboardShortcut("]")
            Button("Outdent") { editor?.indent(outdent: true) }
                .keyboardShortcut("[")
        }
        .disabled(editor == nil)
    }

    private func item(_ title: String, _ formatting: EditorCommand.Formatting, _ key: KeyEquivalent, _ modifiers: EventModifiers = .command) -> some View {
        Button(title) { editor?.format(formatting) }
            .keyboardShortcut(key, modifiers: modifiers)
    }
}

/// The Command palette, Source mode for the Note in front of the window, the writing modes.
private struct ViewCommands: Commands {
    @FocusedValue(\.noteWindow) private var sceneWindow
    private var window: AppModel? { sceneWindow ?? LooseFileFocus.shared.noteWindow }
    @FocusedValue(\.vaultWindow) private var vaultWindow

    var body: some Commands {
        CommandGroup(before: .toolbar) {
            // As VS Code's (⌘P is Print): every command of the menus, found by name.
            Button("Command Palette…") { (vaultWindow ?? window)?.showPalette(in: NSApp.keyWindow) }
                .keyboardShortcut("p", modifiers: [.command, .shift])
                .disabled(vaultWindow ?? window == nil)
            Divider()
            Toggle("Source Mode", isOn: Binding(
                get: { window?.isSourceMode ?? false },
                set: { window?.prefersSourceMode = $0 }
            ))
            .keyboardShortcut("e")
            // A Note too large for Live preview stays in Source mode.
            .disabled(window == nil || window?.isSourceModeForced == true)
            // For every window, kept in Settings as the editor's other choices.
            Toggle("Focus Mode", isOn: Binding(
                get: { AppSettings.shared.editor.focusMode },
                set: { AppSettings.shared.editor = AppSettings.shared.editor.with(focusMode: $0) }
            ))
            Toggle("Typewriter Mode", isOn: Binding(
                get: { AppSettings.shared.editor.typewriterMode },
                set: { AppSettings.shared.editor = AppSettings.shared.editor.with(typewriterMode: $0) }
            ))
            Divider()
        }
    }
}

/// Zoom of the Attachment in front; without one, the text's size, kept in Settings as if
/// changed there, so every window follows.
private struct ZoomCommands: Commands {
    @FocusedValue(\.zoom) private var zoom

    var body: some Commands {
        CommandGroup(after: .toolbar) {
            Button("Zoom In") { if let zoom { zoom.zoomIn() } else { stepTextSize(by: 1) } }
                .keyboardShortcut("+")
            Button("Zoom Out") { if let zoom { zoom.zoomOut() } else { stepTextSize(by: -1) } }
                .keyboardShortcut("-")
            Button("Actual Size") { setTextSize(EditorSettings.standard.size) }
                .keyboardShortcut("0")
                .disabled(zoom != nil)
        }
    }

    private func stepTextSize(by step: Double) {
        setTextSize(AppSettings.shared.editor.size + step)
    }

    @MainActor private func setTextSize(_ size: Double) {
        let settings = AppSettings.shared
        // Kept within the bounds by EditorSettings; past them nothing changes.
        settings.editor = settings.editor.with(size: size)
    }
}
