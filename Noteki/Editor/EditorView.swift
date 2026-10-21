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
    /// The user clicked a Wikilink: what is between its brackets.
    var onFollowLink: (_ inner: String) -> Void = { _ in }
    /// The links of the Note loaded as `noteID`: which lead nowhere?
    var onCheckLinks: (_ noteID: String, _ inners: [String]) -> Void = { _, _ in }
    /// The Notes to offer when the user types `[[`.
    var linkCompletions: () -> LinkCompletions = { .none }

    private var isReady = false
    /// What the editor shows, or will show once it is ready.
    private var loaded: (noteID: String, text: String)?
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

    override init() {
        let configuration = WKWebViewConfiguration()
        webView = EditorWebView(frame: .zero, configuration: configuration)
        super.init()
        webView.onCommand = { [weak self] command in self?.perform(command) }
        // Through a weak proxy: the controller keeps its handlers alive.
        configuration.userContentController.add(WeakMessageHandler(self), name: "noteki")
        webView.underPageBackgroundColor = .textBackgroundColor
        webView.navigationDelegate = self
        loadPage()
    }

    func load(noteID: String, text: String) {
        loaded = (noteID, text)
        guard isReady else { return }
        // Read by scripts/measure.
        run(.load(noteID: noteID, text: text)) { _ in logger.info("Note shown") }
    }

    /// An editing command from the menus, when a Note is in front.
    func perform(_ command: EditorCommand) {
        guard isReady, isInFront else { return }
        run(command) { _ in }
    }

    /// The Edit menu's Find items. Opening the search panel gives it the keyboard, as ⌘F does in
    /// any app; Find Next and Find Previous leave the focus where it is.
    func find(_ step: EditorCommand.FindStep) {
        guard isReady, isInFront else { return }
        run(.find(step)) { _ in }
        if step == .open { webView.window?.makeFirstResponder(webView) }
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
        if applied { loaded = (noteID, text) }
        return applied
    }

    /// The links of the Note shown that lead nowhere, drawn as Unresolved links.
    func markUnresolved(noteID: String, inners: [String]) {
        guard isReady, loaded?.noteID == noteID else { return }
        run(.unresolvedLinks(noteID: noteID, inners: inners)) { _ in }
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
            if let loaded { load(noteID: loaded.noteID, text: loaded.text) }
        case .changed(let noteID, let revision, let text):
            // Late keys of the Note just left must not become what a reload shows.
            if loaded?.noteID == noteID { loaded = (noteID, text) }
            onChange(noteID, revision, text)
        case .followLink(let inner):
            onFollowLink(inner)
        case .checkLinks(let noteID, let inners):
            onCheckLinks(noteID, inners)
        case .completeLink(let requestID):
            run(.linkCompletions(requestID: requestID, completions: linkCompletions())) { _ in }
        }
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
