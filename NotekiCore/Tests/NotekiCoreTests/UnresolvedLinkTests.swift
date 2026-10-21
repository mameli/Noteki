import Foundation
import Testing
import NotekiCore

@Suite("Unresolved links")
struct UnresolvedLinkTests {
    @Test("following an Unresolved link creates the Note in the folder of the Note that holds it")
    func inSourceFolder() throws {
        let folder = try TemporaryVault(files: ["Meetings/Retro.md": "See [[Kickoff]]\n"])
        let vault = Vault(root: folder.url)

        let created = try vault.createNote(for: "Kickoff", from: Note(path: "Meetings/Retro.md"))

        #expect(created == Note(path: "Meetings/Kickoff.md"))
        #expect(try folder.read("Meetings/Kickoff.md") == "")
        #expect(VaultIndex(tree: try vault.tree()).resolve("Kickoff", from: Note(path: "Meetings/Retro.md")) == .note(created))
    }

    @Test("a link with a path creates the Note at that path, folders included")
    func withPath() throws {
        let folder = try TemporaryVault(files: ["Home.md": ""])
        let vault = Vault(root: folder.url)

        #expect(try vault.createNote(for: "Clients/Axo/Plan", from: Note(path: "Home.md")) == Note(path: "Clients/Axo/Plan.md"))
        #expect(try vault.createNote(for: "/Archive/Old.md", from: Note(path: "Home.md")) == Note(path: "Archive/Old.md"))
        #expect(FileManager.default.fileExists(atPath: folder.url.appending(path: "Clients/Axo/Plan.md").path))
    }

    @Test("./ and ../ are relative to the linking Note's folder; no source means the Vault's root")
    func relative() throws {
        let folder = try TemporaryVault(files: ["A/B/Note.md": ""])
        let vault = Vault(root: folder.url)

        #expect(try vault.createNote(for: "./Here", from: Note(path: "A/B/Note.md")) == Note(path: "A/B/Here.md"))
        #expect(try vault.createNote(for: "../Up", from: Note(path: "A/B/Note.md")) == Note(path: "A/Up.md"))
        #expect(try vault.createNote(for: "Top", from: nil) == Note(path: "Top.md"))
    }

    @Test("only Notes are created: not an Attachment's name, not over an existing file, not outside the Vault")
    func refused() throws {
        let folder = try TemporaryVault(files: ["Home.md": "keep me\n", ".obsidian/x.md": ""])
        let vault = Vault(root: folder.url)

        #expect(throws: Vault.CreateError.notANote) { try vault.createNote(for: "photo.png", from: nil) }
        #expect(throws: Vault.CreateError.exists(Note(path: "Home.md"))) { try vault.createNote(for: "Home", from: nil) }
        #expect(throws: Vault.CreateError.outside) { try vault.createNote(for: "../../Escape", from: Note(path: "Home.md")) }
        #expect(throws: Vault.CreateError.outside) { try vault.createNote(for: ".obsidian/New", from: nil) }
        #expect(try folder.read("Home.md") == "keep me\n")
    }

    @Test("names Obsidian refuses on a Mac are refused: a backslash, a colon, no name at all")
    func invalidNames() throws {
        let folder = try TemporaryVault(files: ["Home.md": ""])
        let vault = Vault(root: folder.url)

        #expect(throws: Vault.CreateError.invalidName) { try vault.createNote(for: "a:b", from: nil) }
        #expect(throws: Vault.CreateError.invalidName) { try vault.createNote(for: "a\\b", from: nil) }
        #expect(throws: Vault.CreateError.invalidName) { try vault.createNote(for: "Folder/", from: nil) }
    }

    @Test("an uppercase extension makes a .md Note, which the link then resolves to")
    func uppercaseExtension() throws {
        let folder = try TemporaryVault(files: ["Home.md": ""])
        let vault = Vault(root: folder.url)

        let created = try vault.createNote(for: "Plan.MD", from: nil)

        #expect(created == Note(path: "Plan.md"))
        #expect(VaultIndex(tree: try vault.tree()).resolve("Plan.MD", from: nil) == .note(created))
    }

    @Test("a Note already there under other letter case is the one reported, with its real name")
    func existsWithOtherCase() throws {
        let folder = try TemporaryVault(files: ["Meetings/Kickoff.md": "keep\n"])
        let vault = Vault(root: folder.url)

        #expect(throws: Vault.CreateError.exists(Note(path: "Meetings/Kickoff.md"))) {
            try vault.createNote(for: "kickoff", from: Note(path: "Meetings/Retro.md"))
        }
        #expect(try folder.read("Meetings/Kickoff.md") == "keep\n")
    }
}
