import Foundation
import ImageIO
import UniformTypeIdentifiers

/// The file an Embed shows (CONTEXT.md): an image drawn in the Note, or a PDF linked from it.
public struct EmbeddedFile: Equatable, Sendable, Encodable {
    public enum Kind: String, Sendable, Encodable {
        case image, pdf
    }

    /// From the Vault's root.
    public let path: String
    public let kind: Kind
    /// In pixels, as the image is shown (rotated by its orientation): the editor gives the image
    /// its place before it loads, so the text below does not move. Nil for a PDF, or an image
    /// whose size cannot be read.
    public let width: Int?
    public let height: Int?
    /// When the file last changed, in seconds: the editor asks for the image again when it does.
    public let version: Int?

    public init(path: String, kind: Kind, width: Int?, height: Int?, version: Int? = nil) {
        self.path = path
        self.kind = kind
        self.width = width
        self.height = height
        self.version = version
    }
}

/// An image as the editor gets it: its bytes, and their type.
public struct ServedImage: Sendable {
    public let data: Data
    public let mimeType: String
}

extension Vault {
    /// The image or PDF an Embed shows from `source`: `![[name.png]]` (what is between its brackets)
    /// or a Markdown image's URL, found as a Wikilink is. Nil for a missing file, a Note (not
    /// embedded yet) or a web address.
    public func embed(_ inner: String, from source: Note?, in index: VaultIndex) -> EmbeddedFile? {
        guard !inner.contains("://") else { return nil }
        guard case .attachment(let attachment)? = index.resolve(Wikilink(inner: inner).target, from: source) else { return nil }
        switch attachment.kind {
        case .pdf:
            return EmbeddedFile(path: attachment.path, kind: .pdf, width: nil, height: nil)
        case .image:
            let file = url(of: attachment)
            let size = pixelSize(of: file)
            let changed = (try? file.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate
            return EmbeddedFile(
                path: attachment.path, kind: .image, width: size?.width, height: size?.height,
                version: changed.map { Int($0.timeIntervalSince1970) }
            )
        }
    }

    /// The widest image the editor gets, in pixels: a text 46 rem wide on a Retina screen. WebKit
    /// keeps an image decoded at its full size, 4 bytes a pixel: a 6000-pixel photo would take
    /// 100 MB of the memory budget (SPEC, Budget).
    public static let largestServedImage = 1600

    /// The image the editor asks for by its path, only if it is an image inside the Vault and not
    /// in an Excluded folder: the web view reads nothing else. Larger than `largestServedImage`, it
    /// is served smaller, as a JPEG (a PNG if it has transparency). Blocks while it reads.
    public func servedImage(at path: String) -> ServedImage? {
        guard let attachment = Attachment(path: path), attachment.kind == .image, !isInExcludedFolder(path) else { return nil }
        let file = url(of: attachment).standardizedFileURL.resolvingSymlinksInPath()
        let inside = root.standardizedFileURL.resolvingSymlinksInPath().path(percentEncoded: false)
        let folder = inside.hasSuffix("/") ? inside : inside + "/"
        guard file.path(percentEncoded: false).hasPrefix(folder), let data = try? Data(contentsOf: file) else { return nil }
        let type = UTType(filenameExtension: file.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
        guard let size = pixelSize(of: file), max(size.width, size.height) > Self.largestServedImage else {
            return ServedImage(data: data, mimeType: type)
        }
        return smaller(data) ?? ServedImage(data: data, mimeType: type)
    }

    private func smaller(_ data: Data) -> ServedImage? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: Self.largestServedImage,
        ]
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else { return nil }
        let transparent = ![.none, .noneSkipFirst, .noneSkipLast].contains(image.alphaInfo)
        let type = transparent ? UTType.png : UTType.jpeg
        let output = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(output, type.identifier as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(destination, image, [kCGImageDestinationLossyCompressionQuality: 0.85] as CFDictionary)
        guard CGImageDestinationFinalize(destination) else { return nil }
        return ServedImage(data: output as Data, mimeType: type.preferredMIMEType ?? "image/jpeg")
    }

    /// Read from the file's header, without decoding the image.
    private func pixelSize(of file: URL) -> (width: Int, height: Int)? {
        guard let source = CGImageSourceCreateWithURL(file as CFURL, nil),
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [String: Any],
              let width = properties[kCGImagePropertyPixelWidth as String] as? Int,
              let height = properties[kCGImagePropertyPixelHeight as String] as? Int else { return nil }
        // Orientations 5 to 8 turn the image a quarter: its sides swap.
        let orientation = properties[kCGImagePropertyOrientation as String] as? Int ?? 1
        return orientation >= 5 ? (height, width) : (width, height)
    }
}
