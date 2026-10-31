import NotekiCore
import SwiftUI
import WebKit
import os

private let logger = Logger(subsystem: "app.noteki.noteki", category: "editor")

/// Shows the window's editor. The web view belongs to the EditorController, not to this view,
/// so it lives as long as the window, one per window (ADR 0003).
struct EditorView: NSViewRepresentable {
    let editor: EditorController

    func makeNSView(context: Context) -> EditorWebView { editor.webView }
    func updateNSView(_ webView: EditorWebView, context: Context) {}
}

/// The CodeMirror editor in a web view (ADR 0001), loaded once from the bundled editor.html,
/// and the app's side of the bridge to it.
@MainActor
final class EditorController: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
    let webView: EditorWebView
    /// The user typed: the whole text, tagged with the Note it was loaded as and a revision
    /// that grows with every report.
    var onChange: (_ noteID: String, _ revision: Int, _ text: String) -> Void = { _, _, _ in }
    /// The user clicked a Wikilink: what is between its brackets; with ⌘, for a new Tab.
    var onFollowLink: (_ inner: String, _ newTab: Bool) -> Void = { _, _ in }
    /// The links of the Note loaded as `noteID`: which lead nowhere?
    var onCheckLinks: (_ noteID: String, _ inners: [String]) -> Void = { _, _ in }
    /// The Notes to offer when the user types `[[`.
    var linkCompletions: () -> LinkCompletions = { .none }
    /// The Vault's Tags to offer when the user adds one, given to `answer` once they are read.
    var tagCompletions: (_ answer: @escaping @MainActor ([String]) -> Void) -> Void = { $0([]) }
    /// Whether pasted and dropped images go to `onImages` now; otherwise WebKit gets them.
    var takesImages: () -> Bool = { false }
    /// An image pasted, or image files pasted or dropped at `point`: the app writes them in the
    /// Vault and gives their Embeds to `insert`.
    var onImages: (_ images: EditorWebView.IncomingImages, _ point: EditorCommand.Point?) -> Void = { _, _ in }

    private var isReady = false
    /// What the editor shows, or will show once it is ready.
    private var loaded: (noteID: String, text: String, line: Int?)?
    private let page = Bundle.main.url(forResource: "editor", withExtension: "html")
    /// A flush that gets no answer in this time gives up: quitting must not hang on the editor.
    private static let flushTimeout: Duration = .seconds(2)

    /// WebKit reads these from the app's preferences and leaves them off in a web view: on by
    /// default as in any Mac text field, Edit → Spelling and Grammar turns them off. Registered
    /// once, before any web view exists.
    static func registerSpellingDefaults() {
        UserDefaults.standard.register(defaults: [
            "WebContinuousSpellCheckingEnabled": true,
            "WebAutomaticSpellingCorrectionEnabled": true,
        ])
    }

    /// Whether a Note is in front: otherwise the editor takes no commands and gives up the
    /// keyboard, so ⌘Z cannot change a Note hidden behind an Attachment.
    var isInFront = false {
        didSet {
            webView.takesCommands = isInFront
            if !isInFront, webView.window?.firstResponder === webView {
                webView.window?.makeFirstResponder(nil)
            }
        }
    }

    /// The Vault's images, for the Embeds the editor draws.
    let images = ImageSchemeHandler()

    override init() {
        let configuration = WKWebViewConfiguration()
        configuration.setURLSchemeHandler(images, forURLScheme: ImageSchemeHandler.scheme)
        configuration.userContentController.addUserScript(Self.settingsScript())
        webView = EditorWebView(frame: .zero, configuration: configuration)
        super.init()
        NotificationCenter.default.addObserver(self, selector: #selector(settingsChanged), name: AppSettings.editorChanged, object: nil)
        webView.onCommand = { [weak self] command in self?.perform(command) }
        webView.takesImages = { [weak self] in self?.takesImages() ?? false }
        webView.onImages = { [weak self] images, point in self?.onImages(images, point) }
        // Through a weak proxy: the controller keeps its handlers alive.
        configuration.userContentController.add(WeakMessageHandler(self), name: "noteki")
        webView.underPageBackgroundColor = .textBackgroundColor
        webView.navigationDelegate = self
        loadPage()
    }

    /// Source mode, or Live preview: kept across Notes, and given to the editor once it is ready.
    var isSourceMode = false {
        didSet {
            guard isReady, isSourceMode != oldValue else { return }
            run(.sourceMode(on: isSourceMode)) { _ in }
        }
    }

    /// The settings as the page reads them before it loads (ADR 0004): the first Note is drawn
    /// with them, without a round trip. Also after a reload, as they are now.
    private static func settingsScript() -> WKUserScript {
        let json = (try? AppSettings.shared.editor.json()) ?? "null"
        return WKUserScript(source: "window.notekiSettings = \(json);", injectionTime: .atDocumentStart, forMainFrameOnly: true)
    }

    /// Settings changed: the Note shown is drawn again with them at once.
    @objc private func settingsChanged() {
        // The editor's only user script.
        let scripts = webView.configuration.userContentController
        scripts.removeAllUserScripts()
        scripts.addUserScript(Self.settingsScript())
        guard isReady else { return }
        run(.settings(AppSettings.shared.editor)) { _ in }
    }

    /// Shows a Note, at `line` (from 1) if given: where a Backlink points.
    func load(noteID: String, text: String, line: Int? = nil) {
        loaded = (noteID, text, line)
        guard isReady else { return }
        // Read by scripts/measure.
        run(.load(noteID: noteID, text: text, line: line)) { _ in logger.info("Note shown") }
    }

    /// A Tab back in front: its Note as the editor kept it (cursor, scroll, undo), with `text` as
    /// the app has it now.
    func restore(noteID: String, text: String) {
        loaded = (noteID, text, nil)
        guard isReady else { return }
        run(.restore(noteID: noteID, text: text)) { _ in logger.info("Note shown") }
    }

    /// The editor states of Notes no Tab shows any more.
    func drop(noteIDs: [String]) {
        guard isReady else { return }
        run(.drop(noteIDs: noteIDs)) { _ in }
    }

    /// An editing command from the menus, when a Note is in front.
    func perform(_ command: EditorCommand) {
        guard isReady, isInFront else { return }
        run(command) { _ in }
    }

    /// Gives the editor the keyboard, as after choosing a Note in the Quick switcher.
    func focus() {
        guard isInFront else { return }
        webView.window?.makeFirstResponder(webView)
    }

    /// The Edit menu's Find items. Opening the search panel gives it the keyboard, as ⌘F does in
    /// any app; Find Next and Find Previous leave the focus where it is.
    func find(_ step: EditorCommand.FindStep) {
        guard isReady, isInFront else { return }
        run(.find(step)) { _ in }
        if step == .open { webView.window?.makeFirstResponder(webView) }
    }

    /// The Format menu. ⌘K makes a link to the web address on the pasteboard, if it holds one.
    func format(_ formatting: EditorCommand.Formatting) {
        guard isReady, isInFront else { return }
        var url: String?
        if formatting == .link, let copied = NSPasteboard.general.string(forType: .string)?.trimmingCharacters(in: .whitespacesAndNewlines),
           let address = URL(string: copied), ["http", "https", "mailto"].contains(address.scheme?.lowercased() ?? ""), !copied.contains(" ") {
            url = copied
        }
        run(.format(formatting, url: url)) { _ in }
    }

    /// The Format menu's Indent and Outdent.
    func indent(outdent: Bool) {
        guard isReady, isInFront else { return }
        run(.indent(outdent: outdent)) { _ in }
    }

    /// The Note shown changed on disk: its new text, with the cursor kept where it was. True
    /// when the editor shows it; false when the user typed first and the editor kept their text.
    func reload(noteID: String, revision: Int, text: String) async -> Bool {
        guard isReady, loaded?.noteID == noteID else { return false }
        let applied = await withCheckedContinuation { continuation in
            run(.reload(noteID: noteID, revision: revision, text: text)) { reply in
                continuation.resume(returning: reply == "applied")
            }
        }
        if applied { loaded = (noteID, text, nil) }
        return applied
    }

    /// The cursor at the start of `line` in the Note shown, as for a search result in it.
    func goToLine(noteID: String, line: Int) {
        guard isReady, loaded?.noteID == noteID else { return }
        run(.goToLine(noteID: noteID, line: line)) { _ in }
    }

    /// The links of the Note shown that lead nowhere, drawn as Unresolved links.
    func markUnresolved(noteID: String, inners: [String]) {
        guard isReady, loaded?.noteID == noteID else { return }
        run(.unresolvedLinks(noteID: noteID, inners: inners)) { _ in }
    }

    /// The files the Note's Embeds show: the editor draws the images and links the PDFs.
    func showEmbeds(noteID: String, files: [String: EmbeddedFile]) {
        guard isReady, loaded?.noteID == noteID else { return }
        run(.embeds(noteID: noteID, files: files)) { _ in }
    }

    /// Whether `insert` can write in the Note loaded as `noteID` now.
    func canInsert(into noteID: String) -> Bool {
        isReady && loaded?.noteID == noteID
    }

    /// Writes `text` in the Note shown as if typed: at `point` where files were dropped, otherwise
    /// in place of the selection.
    func insert(noteID: String, text: String, at point: EditorCommand.Point?) {
        guard canInsert(into: noteID) else { return }
        run(.insert(noteID: noteID, text: text, point: point)) { _ in }
    }

    /// Empties the editor and stops it taking typing: no Note is open.
    func clear() {
        loaded = nil
        guard isReady else { return }
        run(.clear) { _ in }
    }

    /// Asks the editor for changes it has not reported yet, and hands them to `onChange`.
    /// Waits at most `flushTimeout`; a reply that comes later is still handed over.
    func flush() async {
        guard isReady else { return }
        await withCheckedContinuation { continuation in
            let once = Once()
            run(.flush) { [weak self] reply in
                if let reply { self?.receive(reply) }
                once.run { continuation.resume() }
            }
            Task {
                try? await Task.sleep(for: Self.flushTimeout)
                once.run {
                    logger.error("The editor did not answer a flush in time")
                    continuation.resume()
                }
            }
        }
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let body = message.body as? String else { return }
        receive(body)
    }

    /// WebKit's process died (memory pressure, a crash): load the editor again and show the
    /// same text once it is ready. Changes not reported yet are lost with the process.
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        logger.error("The editor's web process ended, reloading it")
        isReady = false
        loadPage()
    }

    private func receive(_ json: String) {
        let message: EditorMessage
        do {
            message = try EditorMessage(json: json)
        } catch {
            logger.error("Unreadable message from the editor: \(error.localizedDescription, privacy: .public)")
            return
        }
        switch message {
        case .ready:
            isReady = true
            if isSourceMode { run(.sourceMode(on: true)) { _ in } }
            // Settings changed while the page loaded may have missed its script.
            run(.settings(AppSettings.shared.editor)) { _ in }
            if let loaded { load(noteID: loaded.noteID, text: loaded.text, line: loaded.line) }
        case .changed(let noteID, let revision, let text):
            // Late keys of the Note just left must not become what a reload shows.
            if loaded?.noteID == noteID { loaded = (noteID, text, nil) }
            onChange(noteID, revision, text)
        case .followLink(let inner, let newTab):
            onFollowLink(inner, newTab)
        case .format(let formatting):
            format(formatting)
        case .openURL(let address):
            // Only what the editor promised: the web and mail, never a file or an app's scheme.
            if let url = URL(string: address), ["http", "https", "mailto"].contains(url.scheme?.lowercased() ?? "") {
                NSWorkspace.shared.open(url)
            }
        case .checkLinks(let noteID, let inners):
            onCheckLinks(noteID, inners)
        case .completeLink(let requestID):
            run(.linkCompletions(requestID: requestID, completions: linkCompletions())) { _ in }
        case .addTag(let noteID, let text, let tag):
            edit(.add(tag), noteID: noteID, text: text)
        case .removeTag(let noteID, let text, let tag):
            edit(.remove(tag), noteID: noteID, text: text)
        case .completeTag(let requestID):
            tagCompletions { [weak self] tags in self?.run(.tagCompletions(requestID: requestID, tags: tags)) { _ in } }
        case .readFrontmatter(let noteID, let text):
            // Read here: it needs nothing of the Vault, and a Loose file has Properties too.
            run(.frontmatter(noteID: noteID, text: text, reading: Frontmatter(of: text)?.reading)) { _ in }
        }
    }

    /// The Frontmatter `text` with a Tag added or removed, back to the editor to put in place;
    /// nothing when NotekiCore changes nothing (YAML it cannot read is never rewritten).
    private func edit(_ edit: TagEdit, noteID: String, text: String) {
        guard let replacement = Frontmatter.editingTags(edit, in: text) else { return }
        run(.replaceFrontmatter(noteID: noteID, text: text, replacement: replacement)) { _ in }
    }

    private func loadPage() {
        guard let page else {
            logger.fault("editor.html is missing from the app")
            return
        }
        webView.loadFileURL(page, allowingReadAccessTo: page)
    }

    /// Sends a command; `reply` gets what the editor returned, nil for nothing.
    private func run(_ command: EditorCommand, reply: @escaping @MainActor (String?) -> Void) {
        let json: String
        do {
            json = try command.json()
        } catch {
            logger.error("Cannot encode an editor command: \(error.localizedDescription, privacy: .public)")
            reply(nil)
            return
        }
        webView.callAsyncJavaScript(
            "return window.noteki.receive(JSON.parse(command))",
            arguments: ["command": json], in: nil, in: .page
        ) { result in
            switch result {
            case .success(let value):
                reply(value as? String)
            case .failure(let error):
                logger.error("Editor command failed: \(error.localizedDescription, privacy: .public)")
                reply(nil)
            }
        }
    }
}

/// Runs the first closure it is given, ignores the others: one answer for a continuation that
/// two things race to resume.
@MainActor
private final class Once {
    private var done = false

    func run(_ body: () -> Void) {
        guard !done else { return }
        done = true
        body()
    }
}

private final class WeakMessageHandler: NSObject, WKScriptMessageHandler {
    private weak var target: (any WKScriptMessageHandler)?

    init(_ target: any WKScriptMessageHandler) {
        self.target = target
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        target?.userContentController(controller, didReceive: message)
    }
}
