import AppKit
import NotekiCore
import UniformTypeIdentifiers
import WebKit
import SwiftUI
import os

private let logger = Logger(subsystem: "app.noteki.noteki", category: "export")

/// File → Export as HTML…, Export as PDF… and Print… (SPEC, Export): the Note in front as one
/// page standing alone, drawn by the editor (export.ts) in light colours whatever the theme, its
/// images put in as data. A PDF is paged as a print is, A4 or the Mac's paper.
@MainActor
enum NoteExport {
    enum Kind {
        case html, pdf, print
    }

    /// Asks where to save (not for Print), then writes the page there or prints it.
    static func run(_ kind: Kind, of model: AppModel) {
        guard case .note(let note)? = model.shown, let vault = model.vault else { return NSSound.beep() }
        let window = NSApp.keyWindow
        Task {
            // What was typed last is in the page: the editor gives its own text.
            guard let raw = await model.editor.exportHTML(title: note.name) else {
                return fail("Noteki could not draw “\(note.name)” to export it.")
            }
            let html = withKatexStyle(vault.inliningImages(in: raw))
            switch kind {
            case .html:
                guard let url = askWhere(note.name, type: .html, in: window) else { return }
                do {
                    try html.write(to: url, atomically: true, encoding: .utf8)
                } catch {
                    fail("Cannot save “\(url.lastPathComponent)”: \(error.localizedDescription)")
                }
            case .pdf:
                guard let url = askWhere(note.name, type: .pdf, in: window) else { return }
                await Printer(html: html, title: note.name).run(savingTo: url, sheetOn: window)
            case .print:
                await Printer(html: html, title: note.name).run(savingTo: nil, sheetOn: window)
            }
        }
    }

    /// KaTeX's stylesheet, with its fonts inside, when the page has a formula drawn by KaTeX.
    private static func withKatexStyle(_ html: String) -> String {
        guard html.contains("class=\"katex"),
              let url = Bundle.main.url(forResource: "katex.min", withExtension: "css"),
              let style = try? String(contentsOf: url, encoding: .utf8) else { return html }
        return html.replacingOccurrences(of: "</head>", with: "<style>\(style)</style>\n</head>")
    }

    private static func askWhere(_ name: String, type: UTType, in window: NSWindow?) -> URL? {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [type]
        panel.nameFieldStringValue = name
        panel.canCreateDirectories = true
        return panel.runModal() == .OK ? panel.url : nil
    }

    static func fail(_ message: String) {
        logger.error("\(message, privacy: .public)")
        let alert = NSAlert()
        alert.messageText = message
        alert.runModal()
    }
}

/// A web view out of sight that loads the page, then prints it: to the printer, through the print
/// panel on the Note's window, or to a PDF file, paged, without a panel.
@MainActor
private final class Printer: NSObject, WKNavigationDelegate {
    private let html: String
    private let title: String
    private let webView: WKWebView
    private let window: NSWindow
    private var loaded: CheckedContinuation<Void, Never>?

    init(html: String, title: String) {
        self.html = html
        self.title = title
        let configuration = WKWebViewConfiguration()
        // Nothing of the page runs: it is HTML and CSS, KaTeX and Mermaid already drawn.
        configuration.defaultWebpagePreferences.allowsContentJavaScript = false
        // As wide as an A4 page's text, so the print lays it out as it will be read.
        let frame = NSRect(x: 0, y: 0, width: 800, height: 1000)
        webView = WKWebView(frame: frame, configuration: configuration)
        // Never shown: a window is needed for WebKit to lay out and print the page.
        window = NSWindow(contentRect: frame.offsetBy(dx: -20000, dy: -20000), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        super.init()
        webView.navigationDelegate = self
        window.contentView = webView
    }

    func run(savingTo url: URL?, sheetOn sheetWindow: NSWindow?) async {
        await withCheckedContinuation { continuation in
            loaded = continuation
            webView.loadHTMLString(html, baseURL: nil)
        }
        let info = (NSPrintInfo.shared.copy() as? NSPrintInfo) ?? NSPrintInfo()
        info.horizontalPagination = .fit
        info.verticalPagination = .automatic
        info.isHorizontallyCentered = false
        info.isVerticallyCentered = false
        for margin in [\NSPrintInfo.topMargin, \.bottomMargin] { info[keyPath: margin] = 54 }
        for margin in [\NSPrintInfo.leftMargin, \.rightMargin] { info[keyPath: margin] = 48 }
        if let url {
            info.jobDisposition = .save
            info.dictionary()[NSPrintInfo.AttributeKey.jobSavingURL] = url
        }
        let operation = webView.printOperation(with: info)
        operation.jobTitle = title
        operation.showsPrintPanel = url == nil
        operation.showsProgressPanel = false
        operation.view?.frame = webView.bounds
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            let done = PrintDone { continuation.resume() }
            if url == nil, let sheetWindow {
                operation.runModal(for: sheetWindow, delegate: done, didRun: #selector(PrintDone.printOperationDidRun(_:success:contextInfo:)), contextInfo: nil)
            } else {
                operation.runModal(for: window, delegate: done, didRun: #selector(PrintDone.printOperationDidRun(_:success:contextInfo:)), contextInfo: nil)
            }
        }
        window.close()
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        loaded?.resume()
        loaded = nil
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: any Error) {
        loaded?.resume()
        loaded = nil
    }
}

/// The print operation's end, as AppKit tells it: through an Objective-C selector.
private final class PrintDone: NSObject {
    private var finish: (() -> Void)?
    private var keep: PrintDone?

    init(_ finish: @escaping () -> Void) {
        self.finish = finish
        super.init()
        // Until AppKit calls back: it holds the delegate weakly.
        keep = self
    }

    @objc func printOperationDidRun(_ operation: NSPrintOperation, success: Bool, contextInfo: UnsafeMutableRawPointer?) {
        finish?()
        finish = nil
        keep = nil
    }
}

/// File → Export as HTML…, Export as PDF… and Print… (⌘P), for the Note in front of the window.
struct ExportCommands: Commands {
    var body: some Commands {
        CommandGroup(replacing: .printItem) {
            ExportMenuItems()
        }
    }
}

private struct ExportMenuItems: View {
    @FocusedValue(\.noteWindow) private var sceneWindow
    private var window: AppModel? { sceneWindow ?? LooseFileFocus.shared.noteWindow }

    var body: some View {
        Button("Export as HTML…") { if let window { NoteExport.run(.html, of: window) } }
            .disabled(window == nil)
        Button("Export as PDF…") { if let window { NoteExport.run(.pdf, of: window) } }
            .disabled(window == nil)
        Divider()
        Button("Print…") { if let window { NoteExport.run(.print, of: window) } }
            .keyboardShortcut("p")
            .disabled(window == nil)
    }
}
