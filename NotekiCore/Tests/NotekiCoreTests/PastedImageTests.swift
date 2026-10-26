import Foundation
import Testing
import NotekiCore

@Suite("Pasted images and dropped files")
struct PastedImageTests {
    private let png = Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3])

    /// 26 October 2026, 09:30:05, in the Mac's own time zone, as the name is written.
    private let date = Calendar.current.date(from: DateComponents(year: 2026, month: 10, day: 26, hour: 9, minute: 30, second: 5))!

    /// A file outside the Vault, as the Finder drops it.
    private func outside(_ name: String, _ content: String = "image") throws -> (URL, TemporaryVault) {
        let folder = try TemporaryVault(files: [name: content])
        return (folder.url.appending(path: name), folder)
    }

    @Test("a Pasted image is saved in the Note's folder, named after the moment, and embedded by its name")
    func pasted() throws {
        let folder = try TemporaryVault(files: ["Meetings/Retro.md": ""])
        let vault = Vault(root: folder.url)

        let embed = try vault.pasteImage(png, into: Note(path: "Meetings/Retro.md"), at: date)

        #expect(embed == "![[Pasted image 20261026093005.png]]")
        #expect(try folder.data("Meetings/Pasted image 20261026093005.png") == png)
    }

    @Test("a Note at the Vault's root has its Pasted images at the root")
    func pastedAtRoot() throws {
        let folder = try TemporaryVault(files: ["Home.md": ""])
        let vault = Vault(root: folder.url)

        #expect(try vault.pasteImage(png, into: Note(path: "Home.md"), at: date) == "![[Pasted image 20261026093005.png]]")
        #expect(try folder.data("Pasted image 20261026093005.png") == png)
    }

    @Test("two images pasted in the same second never overwrite each other, nor a name in other letter case")
    func pastedTwice() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "", "pasted image 20261026093005 1.PNG": "keep me"])
        let vault = Vault(root: folder.url)
        let note = Note(path: "Retro.md")

        let first = try vault.pasteImage(png, into: note, at: date)
        let second = try vault.pasteImage(Data([1]), into: note, at: date)

        #expect(first == "![[Pasted image 20261026093005.png]]")
        #expect(second == "![[Pasted image 20261026093005 2.png]]")
        #expect(try folder.data("Pasted image 20261026093005.png") == png)
        #expect(try folder.read("pasted image 20261026093005 1.PNG") == "keep me")
    }

    @Test("an image dropped from outside the Vault is copied next to the Note, keeping its name")
    func dropped() throws {
        let folder = try TemporaryVault(files: ["Meetings/Retro.md": ""])
        let vault = Vault(root: folder.url)
        let (file, source) = try outside("Whiteboard.JPG", "photo")

        let embeds = try vault.dropFiles([file], onto: Note(path: "Meetings/Retro.md"), in: VaultIndex(tree: try vault.tree()))

        #expect(embeds == "![[Whiteboard.JPG]]")
        #expect(try folder.read("Meetings/Whiteboard.JPG") == "photo")
        #expect(try source.read("Whiteboard.JPG") == "photo")
    }

    @Test("a dropped image whose name is taken gets a number, the file already there untouched")
    func droppedTaken() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "", "photo.png": "old", "photo 1.png": "older"])
        let vault = Vault(root: folder.url)
        let (file, source) = try outside("photo.png", "new")

        let embeds = try vault.dropFiles([file], onto: Note(path: "Retro.md"), in: VaultIndex(tree: try vault.tree()))
        withExtendedLifetime(source) {}

        #expect(embeds == "![[photo 2.png]]")
        #expect(try folder.read("photo 2.png") == "new")
        #expect(try folder.read("photo.png") == "old")
        #expect(try folder.read("photo 1.png") == "older")
    }

    @Test("several images dropped together are embedded one per line; other files are left out")
    func droppedSeveral() throws {
        let folder = try TemporaryVault(files: ["Retro.md": ""])
        let vault = Vault(root: folder.url)
        let (a, sourceA) = try outside("a.png")
        let (text, sourceText) = try outside("notes.txt")
        let (b, sourceB) = try outside("b.heic")
        let index = VaultIndex(tree: try vault.tree())

        let embeds = try vault.dropFiles([a, text, b], onto: Note(path: "Retro.md"), in: index)
        let nothing = try vault.dropFiles([text], onto: Note(path: "Retro.md"), in: index)
        withExtendedLifetime([sourceA, sourceText, sourceB]) {}

        #expect(embeds == "![[a.png]]\n![[b.heic]]")
        #expect(nothing == nil)
        #expect(!FileManager.default.fileExists(atPath: folder.url.appending(path: "notes.txt").path))
    }

    @Test("an image that cannot be copied is left out, the others embedded; none at all is an error")
    func droppedUnreadable() throws {
        let folder = try TemporaryVault(files: ["Retro.md": ""])
        let vault = Vault(root: folder.url)
        let (a, source) = try outside("a.png")
        try source.write("secret", at: "locked.png")
        let locked = source.url.appending(path: "locked.png")
        try FileManager.default.setAttributes([.posixPermissions: 0], ofItemAtPath: locked.path)
        defer { try? FileManager.default.setAttributes([.posixPermissions: 0o644], ofItemAtPath: locked.path) }
        let index = VaultIndex(tree: try vault.tree())

        let embeds = try vault.dropFiles([locked, a], onto: Note(path: "Retro.md"), in: index)

        #expect(embeds == "![[a.png]]")
        #expect(throws: (any Error).self) { try vault.dropFiles([locked], onto: Note(path: "Retro.md"), in: index) }
        #expect(!FileManager.default.fileExists(atPath: folder.url.appending(path: "locked.png").path))
    }

    @Test("an image already in the Vault is embedded where it is: by its name, or its path when the name leads elsewhere")
    func droppedFromTheVault() throws {
        let folder = try TemporaryVault(files: ["Meetings/Retro.md": "", "Media/chart.png": "a", "Media/logo.png": "b", "Meetings/logo.png": "c"])
        let vault = Vault(root: folder.url)
        let index = VaultIndex(tree: try vault.tree())
        let before = try folder.snapshot()

        let embeds = try vault.dropFiles(
            [folder.url.appending(path: "Media/chart.png"), folder.url.appending(path: "Media/logo.png")],
            onto: Note(path: "Meetings/Retro.md"), in: index
        )

        #expect(embeds == "![[chart.png]]\n![[Media/logo.png]]")
        #expect(try folder.snapshot() == before)
    }

    @Test("an image in an Excluded folder is copied: the Vault does not list it")
    func droppedFromAnExcludedFolder() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "", ".trash/old.png": "a"])
        let vault = Vault(root: folder.url)

        let embeds = try vault.dropFiles([folder.url.appending(path: ".trash/old.png")], onto: Note(path: "Retro.md"), in: VaultIndex(tree: try vault.tree()))

        #expect(embeds == "![[old.png]]")
        #expect(try folder.read("old.png") == "a")
    }

    @Test("characters a Wikilink cannot hold are left out of a copied image's name")
    func droppedOddName() throws {
        let folder = try TemporaryVault(files: ["Retro.md": ""])
        let vault = Vault(root: folder.url)
        let (file, source) = try outside("[draft] plan #2|v^3.png")

        let embeds = try vault.dropFiles([file], onto: Note(path: "Retro.md"), in: VaultIndex(tree: try vault.tree()))
        withExtendedLifetime(source) {}

        #expect(embeds == "![[draft plan 2 v 3.png]]")
        #expect(FileManager.default.fileExists(atPath: folder.url.appending(path: "draft plan 2 v 3.png").path))
    }
}
