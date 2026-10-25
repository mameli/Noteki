import Foundation
import NotekiCore
import WebKit

/// Serves the Vault's images to the editor under `noteki-file://vault/<path>`: an image inside
/// the Vault and nothing else (`Vault.servedImage`). The page itself is a file the web view may read
/// alone; the Vault stays out of its reach.
final class ImageSchemeHandler: NSObject, WKURLSchemeHandler {
    static let scheme = "noteki-file"

    /// The open Vault; nil before one is open, and in a Loose file's window, which embeds nothing.
    @MainActor var vault: Vault?
    /// Requests the web view stopped: their answer, read away from the main thread, is dropped.
    @MainActor private var stopped: Set<ObjectIdentifier> = []

    @MainActor
    func webView(_ webView: WKWebView, start task: any WKURLSchemeTask) {
        guard let vault, let url = task.request.url else {
            task.didFailWithError(URLError(.fileDoesNotExist))
            return
        }
        let path = String(url.path(percentEncoded: false).drop { $0 == "/" })
        let id = ObjectIdentifier(task)
        Task {
            // An image of several MB, perhaps made smaller: away from the main thread.
            let served = await Task.detached { vault.servedImage(at: path) }.value
            defer { stopped.remove(id) }
            guard !stopped.contains(id) else { return }
            guard let served else { return task.didFailWithError(URLError(.fileDoesNotExist)) }
            task.didReceive(URLResponse(url: url, mimeType: served.mimeType, expectedContentLength: served.data.count, textEncodingName: nil))
            task.didReceive(served.data)
            task.didFinish()
        }
    }

    @MainActor
    func webView(_ webView: WKWebView, stop task: any WKURLSchemeTask) {
        stopped.insert(ObjectIdentifier(task))
    }
}
