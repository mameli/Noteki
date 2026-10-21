import SwiftUI
import WebKit
import os

private let logger = Logger(subsystem: "app.noteki.noteki", category: "editor")

/// The CodeMirror editor in a web view (ADR 0001), loaded once from the bundled editor.html.
struct EditorView: NSViewRepresentable {
    let text: String

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> WKWebView {
        context.coordinator.webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        context.coordinator.show(text)
    }

    @MainActor
    final class Coordinator: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
        let webView: WKWebView
        private var isReady = false
        private var shownText: String?
        private var pendingText: String?
        private let page = Bundle.main.url(forResource: "editor", withExtension: "html")

        override init() {
            let configuration = WKWebViewConfiguration()
            webView = WKWebView(frame: .zero, configuration: configuration)
            super.init()
            // Through a weak proxy: the controller keeps its handlers alive.
            configuration.userContentController.add(WeakMessageHandler(self), name: "noteki")
            webView.underPageBackgroundColor = .textBackgroundColor
            webView.navigationDelegate = self
            loadPage()
        }

        private func loadPage() {
            guard let page else {
                logger.fault("editor.html is missing from the app")
                return
            }
            webView.loadFileURL(page, allowingReadAccessTo: page)
        }

        /// WebKit's process died (memory pressure, a crash): load the editor again and show the
        /// same text once it is ready.
        func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
            logger.error("The editor's web process ended, reloading it")
            isReady = false
            pendingText = pendingText ?? shownText
            shownText = nil
            loadPage()
        }

        func show(_ text: String) {
            guard text != shownText else { return }
            guard isReady else {
                pendingText = text
                return
            }
            shownText = text
            send(.load(text: text))
        }

        func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
            guard let body = message.body as? String,
                  let decoded = try? JSONDecoder().decode(EditorMessage.self, from: Data(body.utf8))
            else {
                logger.error("Unreadable message from the editor")
                return
            }
            switch decoded {
            case .ready:
                isReady = true
                if let pendingText {
                    self.pendingText = nil
                    show(pendingText)
                }
            }
        }

        private func send(_ command: EditorCommand) {
            let json: String
            do {
                json = String(decoding: try JSONEncoder().encode(command), as: UTF8.self)
            } catch {
                logger.error("Cannot encode an editor command: \(error.localizedDescription, privacy: .public)")
                return
            }
            webView.callAsyncJavaScript(
                "window.noteki.receive(JSON.parse(command))",
                arguments: ["command": json], in: nil, in: .page
            ) { result in
                if case .failure(let error) = result {
                    logger.error("Editor command failed: \(error.localizedDescription, privacy: .public)")
                }
            }
        }
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
