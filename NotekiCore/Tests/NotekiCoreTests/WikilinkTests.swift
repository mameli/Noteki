import Foundation
import Testing
import NotekiCore

/// Resolution as in Obsidian 1.14.4 (`MetadataCache.getLinkpathDest`), read from its code.
@Suite("Wikilinks")
struct WikilinkTests {
    private func index(_ files: [String]) throws -> (VaultIndex, TemporaryVault) {
        let folder = try TemporaryVault(files: Dictionary(uniqueKeysWithValues: files.map { ($0, "") }))
        return (VaultIndex(tree: try Vault(root: folder.url).tree()), folder)
    }

    @Test("a name resolves to the only Note with that file name, wherever it is, in any case")
    func uniqueName() throws {
        let (index, _) = try index(["Meetings/2026/Retro.md", "Home.md"])

        #expect(index.resolve("Retro", from: Note(path: "Home.md")) == .note(Note(path: "Meetings/2026/Retro.md")))
        #expect(index.resolve("retro", from: Note(path: "Home.md")) == .note(Note(path: "Meetings/2026/Retro.md")))
        #expect(index.resolve("Retro.md", from: Note(path: "Home.md")) == .note(Note(path: "Meetings/2026/Retro.md")))
        #expect(index.resolve("Missing", from: Note(path: "Home.md")) == nil)
    }

    @Test("the alias, the heading and the block are not part of the name")
    func targetOnly() {
        #expect(Wikilink(inner: "Retro|the retro").target == "Retro")
        #expect(Wikilink(inner: "Retro|the retro").alias == "the retro")
        #expect(Wikilink(inner: "Retro#Decisions").target == "Retro")
        #expect(Wikilink(inner: "Retro#^block|see").target == "Retro")
        #expect(Wikilink(inner: "  Retro  ").target == "Retro")
        // In a table the pipe is escaped, as Obsidian writes it.
        #expect(Wikilink(inner: "Retro\\|the retro").target == "Retro")
        #expect(Wikilink(inner: "Retro\\|the retro").alias == "the retro")
    }

    @Test("a link with no name, only a heading or an Alias, leads to the Note that holds it")
    func noName() throws {
        let (index, _) = try index(["Meetings/Retro.md", "Home.md"])
        let retro = Note(path: "Meetings/Retro.md")

        #expect(index.resolve(Wikilink(inner: "#Decisions").target, from: retro) == .note(retro))
        #expect(index.resolve("", from: nil) == nil)
    }

    @Test("among Notes with the same name and path length, the first in Files wins")
    func ties() throws {
        let (index, _) = try index(["b/Retro.md", "A/Retro.md", "Home.md"])

        #expect(index.resolve("Retro", from: Note(path: "Home.md")) == .note(Note(path: "A/Retro.md")))
    }

    @Test("a path resolves from the Vault's root, with or without a leading slash or .md")
    func paths() throws {
        let (index, _) = try index(["Meetings/Retro.md", "Archive/Retro.md", "Home.md"])
        let home = Note(path: "Home.md")

        #expect(index.resolve("Archive/Retro", from: home) == .note(Note(path: "Archive/Retro.md")))
        #expect(index.resolve("/Archive/Retro.md", from: home) == .note(Note(path: "Archive/Retro.md")))
        #expect(index.resolve("archive/retro", from: home) == .note(Note(path: "Archive/Retro.md")))
        #expect(index.resolve("/Elsewhere/Retro", from: home) == nil)
    }

    @Test("./ and ../ resolve from the folder of the Note that holds the link")
    func relative() throws {
        let (index, _) = try index(["A/B/Note.md", "A/Retro.md", "A/B/Retro.md"])
        let note = Note(path: "A/B/Note.md")

        #expect(index.resolve("./Retro", from: note) == .note(Note(path: "A/B/Retro.md")))
        #expect(index.resolve("../Retro", from: note) == .note(Note(path: "A/Retro.md")))
    }

    @Test("among Notes with the same name, those under the linking Note's folder come first, then the shortest path")
    func duplicates() throws {
        let (index, _) = try index([
            "Clients/Axo/Retro.md", "Clients/Axo/Old/Retro.md", "Retro2/Retro.md", "Zeta/Deep/Down/Retro.md",
            "Clients/Axo/Kickoff.md", "Home.md",
        ])

        #expect(index.resolve("Retro", from: Note(path: "Clients/Axo/Kickoff.md")) == .note(Note(path: "Clients/Axo/Retro.md")))
        #expect(index.resolve("Retro", from: Note(path: "Home.md")) == .note(Note(path: "Retro2/Retro.md")))
        #expect(index.resolve("Axo/Old/Retro", from: Note(path: "Home.md")) == .note(Note(path: "Clients/Axo/Old/Retro.md")))
    }

    @Test("Attachments resolve by their file name with the extension, as Embeds write them")
    func attachments() throws {
        let (index, _) = try index(["Media/Pasted image 20260917102056.png", "Home.md"])

        #expect(index.resolve("Pasted image 20260917102056.png", from: Note(path: "Home.md"))
            == .attachment(Attachment(path: "Media/Pasted image 20260917102056.png")!))
    }

    @Test("the index built from the listing after a change follows the files created and deleted")
    func followsChanges() throws {
        let folder = try TemporaryVault(files: ["Home.md": ""])
        let vault = Vault(root: folder.url)
        #expect(VaultIndex(tree: try vault.tree()).resolve("Retro", from: Note(path: "Home.md")) == nil)

        try folder.write("# Retro\n", at: "Meetings/Retro.md")
        #expect(VaultIndex(tree: try vault.tree()).resolve("Retro", from: Note(path: "Home.md")) == .note(Note(path: "Meetings/Retro.md")))

        try folder.delete("Meetings/Retro.md")
        #expect(VaultIndex(tree: try vault.tree()).resolve("Retro", from: Note(path: "Home.md")) == nil)
    }
}
