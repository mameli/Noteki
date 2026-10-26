import AppKit
import NotekiCore
import WebKit

/// The editor's web view, handing to CodeMirror the Edit menu actions WebKit would get wrong:
/// CodeMirror draws only the lines on screen, so WebKit's select-all and undo would see a part
/// of the Note. Adapted from MarkEdit's EditorWebView (MIT, see THIRD_PARTY.md).
final class EditorWebView: WKWebView {
    var onCommand: (EditorCommand) -> Void = { _ in }
    /// False while no Note is in front (an Attachment, a placeholder): the menus leave it alone.
    var takesCommands = false
    /// Whether the app writes pasted and dropped images in the Vault now; otherwise WebKit gets them.
    var takesImages: () -> Bool = { false }
    /// An image pasted, or image files pasted or dropped, with where they were dropped: the app
    /// writes them in the Vault and their Embeds in the Note.
    var onImages: (IncomingImages, EditorCommand.Point?) -> Void = { _, _ in }

    /// What the app takes from a paste or a drop (SPEC, Pasted image).
    enum IncomingImages {
        /// A Pasted image, as a screenshot copies it: as PNG.
        case pasted(png: Data)
        /// Image files from the Finder, dropped or copied there.
        case files([URL])
    }

    override func selectAll(_ sender: Any?) {
        onCommand(.selectAll)
    }

    @objc func undo(_ sender: Any?) {
        onCommand(.undo)
    }

    @objc func redo(_ sender: Any?) {
        onCommand(.redo)
    }

    /// WKWebView implements `paste:` without declaring it to Swift: its own is called through
    /// the runtime when the pasteboard holds no image for the app.
    @objc func paste(_ sender: Any?) {
        if takesCommands, takesImages(), let images = Self.images(on: .general) { return onImages(images, nil) }
        let selector = #selector(paste(_:))
        typealias Paste = @convention(c) (AnyObject, Selector, Any?) -> Void
        unsafeBitCast(class_getMethodImplementation(WKWebView.self, selector), to: Paste.self)(self, selector, sender)
    }

    override func validateUserInterfaceItem(_ item: any NSValidatedUserInterfaceItem) -> Bool {
        switch item.action {
        case #selector(undo(_:)), #selector(redo(_:)), #selector(selectAll(_:)): takesCommands
        // From the types alone: the menus validate often, an image is decoded only when pasted.
        case #selector(paste(_:)) where takesCommands && takesImages() && Self.mayHoldImages(.general): true
        default: super.validateUserInterfaceItem(item)
        }
    }

    override func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation {
        dropOperation(sender, webKit: super.draggingEntered(sender))
    }

    override func draggingUpdated(_ sender: any NSDraggingInfo) -> NSDragOperation {
        dropOperation(sender, webKit: super.draggingUpdated(sender))
    }

    /// A copy, for image files the app takes; otherwise what WebKit would do.
    private func dropOperation(_ sender: any NSDraggingInfo, webKit: NSDragOperation) -> NSDragOperation {
        takesCommands && takesImages() && Self.imageFiles(on: sender.draggingPasteboard) != nil ? .copy : webKit
    }

    override func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        guard takesCommands, takesImages(), let files = Self.imageFiles(on: sender.draggingPasteboard) else {
            return super.performDragOperation(sender)
        }
        // WebKit drew its caret where the files would go: it is told the drag left.
        super.draggingExited(sender)
        // The page is not zoomed: a point of the view is a CSS pixel.
        let local = convert(sender.draggingLocation, from: nil)
        onImages(.files(files), EditorCommand.Point(x: local.x, y: isFlipped ? local.y : bounds.height - local.y))
        return true
    }

    /// Image files first, as the Finder copies them (with their names as text); then text, which a
    /// copy from a document or a table holds with a picture of itself; then an image alone.
    private static func images(on pasteboard: NSPasteboard) -> IncomingImages? {
        if let files = imageFiles(on: pasteboard) { return .files(files) }
        guard pasteboard.string(forType: .string) == nil else { return nil }
        if let png = pasteboard.data(forType: .png) { return .pasted(png: png) }
        guard let image = NSImage(pasteboard: pasteboard), let tiff = image.tiffRepresentation,
              let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) else { return nil }
        return .pasted(png: png)
    }

    /// Whether `images(on:)` may find some, without reading them.
    private static func mayHoldImages(_ pasteboard: NSPasteboard) -> Bool {
        imageFiles(on: pasteboard) != nil
            || pasteboard.availableType(from: [.string]) == nil && NSImage.canInit(with: pasteboard)
    }

    /// The files on the pasteboard, when at least one is an image.
    private static func imageFiles(on pasteboard: NSPasteboard) -> [URL]? {
        let files = pasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL] ?? []
        return files.contains { Attachment(path: $0.lastPathComponent)?.kind == .image } ? files : nil
    }
}
