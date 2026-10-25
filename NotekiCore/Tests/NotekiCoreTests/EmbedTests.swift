import Foundation
import ImageIO
import Testing
import UniformTypeIdentifiers
import NotekiCore

@Suite("Embeds")
struct EmbedTests {
    /// A PNG of `width` × `height` pixels.
    private func png(width: Int, height: Int) throws -> Data {
        let context = try #require(CGContext(
            data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ))
        let image = try #require(context.makeImage())
        let data = NSMutableData()
        let destination = try #require(CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(destination, image, nil)
        #expect(CGImageDestinationFinalize(destination))
        return data as Data
    }

    /// The folder is deleted with the TemporaryVault: keep it until the files have been read.
    private func vault(images: [String: (Int, Int)], files: [String: String] = [:]) throws -> (Vault, VaultIndex, TemporaryVault) {
        let folder = try TemporaryVault(files: files)
        for (path, size) in images {
            try FileManager.default.createDirectory(
                at: folder.url.appending(path: path).deletingLastPathComponent(), withIntermediateDirectories: true
            )
            try png(width: size.0, height: size.1).write(to: folder.url.appending(path: path))
        }
        let vault = Vault(root: folder.url)
        return (vault, VaultIndex(tree: try vault.tree()), folder)
    }

    @Test("an Embed of an image is its file and its size in pixels, found as a Wikilink is")
    func image() throws {
        let (vault, index, folder) = try vault(images: ["Media/Pasted image 1.png": (640, 480)], files: ["Note.md": ""])

        let embed = vault.embed("Pasted image 1.png", from: Note(path: "Note.md"), in: index)
        withExtendedLifetime(folder) {}

        #expect(embed?.path == "Media/Pasted image 1.png" && embed?.kind == .image)
        #expect(embed?.width == 640 && embed?.height == 480)
        #expect(embed?.version != nil)
    }

    @Test("a Markdown image's URL is a path (the editor decodes its %20); a web address is not a file")
    func markdownImage() throws {
        let (vault, index, folder) = try vault(images: ["Media/a b.png": (10, 20)], files: ["Note.md": ""])

        #expect(vault.embed("Media/a b.png", from: Note(path: "Note.md"), in: index)?.path == "Media/a b.png")
        #expect(vault.embed("https://example.com/a.png", from: Note(path: "Note.md"), in: index) == nil)
        withExtendedLifetime(folder) {}
    }

    @Test("a PDF is embedded without a size; a Note, a missing file or a broken image is not embedded")
    func others() throws {
        let (vault, index, folder) = try vault(images: [:], files: ["Note.md": "", "Other.md": "", "Doc.pdf": "%PDF-1.4"])
        try Data("not a png".utf8).write(to: folder.url.appending(path: "Broken.png"))
        let index2 = VaultIndex(tree: try vault.tree())

        let pdf = vault.embed("Doc.pdf", from: Note(path: "Note.md"), in: index)
        #expect(pdf?.path == "Doc.pdf" && pdf?.kind == .pdf && pdf?.width == nil)
        #expect(vault.embed("Other", from: Note(path: "Note.md"), in: index) == nil)
        #expect(vault.embed("Missing.png", from: Note(path: "Note.md"), in: index) == nil)
        let broken = vault.embed("Broken.png", from: Note(path: "Note.md"), in: index2)
        #expect(broken?.kind == .image && broken?.width == nil)
    }

    @Test("an Alias or a size after | does not change the file")
    func alias() throws {
        let (vault, index, folder) = try vault(images: ["a.png": (8, 8)], files: ["Note.md": ""])

        #expect(vault.embed("a.png|300", from: Note(path: "Note.md"), in: index)?.path == "a.png")
        withExtendedLifetime(folder) {}
    }

    @Test("only an image inside the Vault is served to the editor")
    func served() throws {
        let (vault, _, folder) = try vault(images: ["Media/a.png": (4, 4)], files: ["Note.md": "secret"])
        let outside = "outside-\(UUID()).png"
        try png(width: 2, height: 2).write(to: folder.url.deletingLastPathComponent().appending(path: outside))
        defer { try? FileManager.default.removeItem(at: folder.url.deletingLastPathComponent().appending(path: outside)) }

        #expect(vault.servedImage(at: "Media/a.png")?.data == (try Data(contentsOf: folder.url.appending(path: "Media/a.png"))))
        #expect(vault.servedImage(at: "Note.md") == nil)
        #expect(vault.servedImage(at: "../\(outside)") == nil)
        #expect(vault.servedImage(at: "Media/../../\(outside)") == nil)
        #expect(vault.servedImage(at: "Missing.png") == nil)
        // Not from an Excluded folder either, as Files does not list it.
        try png(width: 2, height: 2).write(to: folder.url.appending(path: ".trash-x.png"))
        try FileManager.default.createDirectory(at: folder.url.appending(path: ".obsidian"), withIntermediateDirectories: true)
        try png(width: 2, height: 2).write(to: folder.url.appending(path: ".obsidian/icon.png"))
        #expect(vault.servedImage(at: ".obsidian/icon.png") == nil)
    }

    @Test("a large image is served smaller, at most as wide as a text on a Retina screen; a small one as it is")
    func downsampled() throws {
        let (vault, _, folder) = try vault(images: ["big.png": (3000, 1500), "small.png": (300, 200)])

        let big = try #require(vault.servedImage(at: "big.png"))
        let image = try #require(CGImageSourceCreateWithData(big.data as CFData, nil).flatMap { CGImageSourceCreateImageAtIndex($0, 0, nil) })
        #expect(image.width == Vault.largestServedImage && image.height == Vault.largestServedImage / 2)
        #expect(vault.servedImage(at: "small.png")?.data == (try Data(contentsOf: folder.url.appending(path: "small.png"))))
    }

    @Test("an image changed on disk has another version, so the editor loads it again")
    func version() throws {
        let (vault, index, folder) = try vault(images: ["a.png": (4, 4)], files: ["Note.md": ""])
        let before = vault.embed("a.png", from: Note(path: "Note.md"), in: index)?.version
        try FileManager.default.setAttributes([.modificationDate: Date(timeIntervalSinceNow: 60)], ofItemAtPath: folder.url.appending(path: "a.png").path)

        #expect(vault.embed("a.png", from: Note(path: "Note.md"), in: index)?.version != before)
    }
}
