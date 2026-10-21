import NotekiCore
import PDFKit
import SwiftUI

/// An Attachment in a native view: an image scaled to fit, never above its own size, or a PDF
/// in PDFKit with its scrolling and zoom.
struct AttachmentView: View {
    let attachment: Attachment
    let url: URL

    var body: some View {
        switch attachment.kind {
        case .image: ImageAttachment(url: url)
        case .pdf: PDFAttachment(url: url)
        }
    }
}

private struct ImageAttachment: View {
    let url: URL
    @State private var loaded: Loaded<NSImage> = .loading

    var body: some View {
        Group {
            switch loaded {
            case .loading:
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            case .unreadable:
                Unreadable(name: url.lastPathComponent, kind: "an image")
            case .ready(let image):
                Image(nsImage: image)
                    .resizable()
                    .scaledToFit()
                    .frame(maxWidth: image.size.width, maxHeight: image.size.height)
                    .padding()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .task(id: url) {
            loaded = .loading
            // Read once and off the main thread: an image can weigh megabytes.
            let data = await Task.detached { try? Data(contentsOf: url) }.value
            guard !Task.isCancelled else { return }
            loaded = Loaded(data.flatMap(NSImage.init(data:)))
        }
    }
}

private struct PDFAttachment: View {
    let url: URL
    @State private var loaded: Loaded<PDFDocument> = .loading
    @State private var pdfView = PDFView()

    var body: some View {
        Group {
            switch loaded {
            case .loading:
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            case .unreadable:
                Unreadable(name: url.lastPathComponent, kind: "a PDF")
            case .ready(let document):
                PDFKitView(view: pdfView, document: document)
            }
        }
        .focusedSceneValue(\.zoom, ZoomActions(
            zoomIn: { [pdfView] in pdfView.zoomIn(nil) },
            zoomOut: { [pdfView] in pdfView.zoomOut(nil) }
        ))
        .task(id: url) {
            loaded = .loading
            let data = await Task.detached { try? Data(contentsOf: url) }.value
            guard !Task.isCancelled else { return }
            loaded = Loaded(data.flatMap(PDFDocument.init(data:)))
        }
    }
}

private enum Loaded<Content> {
    case loading, unreadable, ready(Content)

    init(_ content: Content?) {
        self = content.map(Loaded.ready) ?? .unreadable
    }
}

private struct Unreadable: View {
    let name: String
    let kind: String

    var body: some View {
        ContentUnavailableView(
            "Cannot Show \(name)", systemImage: "exclamationmark.triangle",
            description: Text("It is not \(kind) macOS can read.")
        )
    }
}

private struct PDFKitView: NSViewRepresentable {
    /// Made by the caller, which also zooms it from the View menu.
    let view: PDFView
    let document: PDFDocument

    func makeNSView(context: Context) -> PDFView {
        view.autoScales = true
        view.displayMode = .singlePageContinuous
        return view
    }

    func updateNSView(_ view: PDFView, context: Context) {
        if view.document !== document { view.document = document }
    }
}
