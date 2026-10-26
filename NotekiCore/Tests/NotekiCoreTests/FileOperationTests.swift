import Foundation
import Testing
import NotekiCore

@Suite("File operations")
struct FileOperationTests {
    @Test("a new Note is Untitled, then Untitled 1, Untitled 2… in the folder asked for, and empty")
    func newNote() throws {
        let folder = try TemporaryVault(files: ["Meetings/Retro.md": "", "Meetings/untitled 1.md": "keep me"])
        let vault = Vault(root: folder.url)

        #expect(try vault.newNote(in: "Meetings") == Note(path: "Meetings/Untitled.md"))
        #expect(try vault.newNote(in: "Meetings") == Note(path: "Meetings/Untitled 2.md"))
        #expect(try vault.newNote(in: "") == Note(path: "Untitled.md"))
        #expect(try folder.read("Meetings/Untitled.md") == "")
        #expect(try folder.read("Meetings/untitled 1.md") == "keep me")
    }

    @Test("a new folder is Untitled, then Untitled 1…, listed though empty; a Note Untitled.md is no folder")
    func newFolder() throws {
        let folder = try TemporaryVault(files: ["Meetings/Retro.md": "", "Untitled/a.md": "", "Untitled.md": ""])
        let vault = Vault(root: folder.url)

        #expect(try vault.newFolder(in: "") == "Untitled 1")
        #expect(try vault.newFolder(in: "Meetings") == "Meetings/Untitled")
        #expect(try vault.tree().contains(.folder(path: "Untitled 1", children: [])))
    }

    @Test("a Note is renamed by its name, its .md kept; an Attachment keeps its extension; a folder takes the name")
    func rename() throws {
        let folder = try TemporaryVault(files: ["Meetings/Retro.md": "text", "Media/photo.png": "png", "Old/a.md": ""])
        let vault = Vault(root: folder.url)

        #expect(try vault.rename("Meetings/Retro.md", to: "Retro 2026") == "Meetings/Retro 2026.md")
        #expect(try vault.rename("Meetings/Retro 2026.md", to: " Plan.md ") == "Meetings/Plan.md")
        #expect(try vault.rename("Media/photo.png", to: "Whiteboard") == "Media/Whiteboard.png")
        #expect(try vault.rename("Old", to: "Archive") == "Archive")
        #expect(try folder.read("Meetings/Plan.md") == "text")
        #expect(try folder.read("Archive/a.md") == "")
    }

    @Test("a rename that only changes letter case is a rename, not a name taken")
    func renameCase() throws {
        let folder = try TemporaryVault(files: ["retro.md": "text"])
        let vault = Vault(root: folder.url)

        #expect(try vault.rename("retro.md", to: "Retro") == "Retro.md")
        #expect(try FileManager.default.contentsOfDirectory(atPath: folder.url.path).contains("Retro.md"))
    }

    @Test("a rename never takes an existing name, nor one Obsidian refuses or the Vault would hide")
    func renameRefused() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "mine", "Plan.md": "theirs", "Media/a.png": ""])
        let vault = Vault(root: folder.url)

        #expect(throws: Vault.FileError.exists("Plan.md")) { try vault.rename("Retro.md", to: "plan") }
        for name in ["", "  ", "a/b", "a:b", "a\\b", ".hidden", "a#b", "[a]", "a|b", "a^b"] {
            #expect(throws: Vault.FileError.invalidName) { try vault.rename("Retro.md", to: name) }
        }
        #expect(throws: Vault.FileError.invalidName) { try vault.rename("Media", to: "node_modules") }
        #expect(try folder.read("Retro.md") == "mine")
        #expect(try folder.read("Plan.md") == "theirs")
    }

    @Test("a Note or a folder moves into another folder, or to the root; into the same one is no move")
    func move() throws {
        let folder = try TemporaryVault(files: ["Inbox/Retro.md": "text", "Inbox/Old/a.md": "", "Meetings/x.md": ""])
        let vault = Vault(root: folder.url)

        #expect(try vault.move("Inbox/Retro.md", into: "Meetings") == "Meetings/Retro.md")
        #expect(try vault.move("Inbox/Old", into: "") == "Old")
        #expect(try vault.move("Meetings/x.md", into: "Meetings") == "Meetings/x.md")
        #expect(try folder.read("Meetings/Retro.md") == "text")
        #expect(try folder.read("Old/a.md") == "")
    }

    @Test("a move never goes over a file with the name, nor puts a folder inside itself")
    func moveRefused() throws {
        let folder = try TemporaryVault(files: ["Inbox/Retro.md": "mine", "Meetings/retro.md": "theirs", "A/B/c.md": ""])
        let vault = Vault(root: folder.url)

        #expect(throws: Vault.FileError.exists("Meetings/retro.md")) { try vault.move("Inbox/Retro.md", into: "Meetings") }
        #expect(throws: Vault.FileError.intoItself) { try vault.move("A", into: "A/B") }
        #expect(throws: Vault.FileError.intoItself) { try vault.move("A", into: "A") }
        #expect(try folder.read("Meetings/retro.md") == "theirs")
    }

    @Test("an open Note moved by Noteki is told where it went, and saves there")
    func openNoteFollows() throws {
        let folder = try TemporaryVault(files: ["Inbox/Retro.md": "text"])
        let vault = Vault(root: folder.url)
        var open = try vault.open(Note(path: "Inbox/Retro.md"))
        open.edit("text typed")

        let moved = try vault.rename("Inbox", to: "Meetings")
        open.moved(from: "Inbox", to: moved)
        try open.save()

        #expect(open.note == Note(path: "Meetings/Retro.md"))
        #expect(try folder.read("Meetings/Retro.md") == "text typed")
        #expect(try open.noticeChanges(at: ["Meetings/Retro.md", "Inbox/Retro.md"]) == .unchanged)
    }

    @Test("only what is inside a moved folder follows it, not a folder whose name starts the same")
    func pathsAfterMove() {
        #expect(pathAfterMove("Inbox/a.md", from: "Inbox", to: "Done") == "Done/a.md")
        #expect(pathAfterMove("Inbox", from: "Inbox", to: "Done") == "Done")
        #expect(pathAfterMove("Inbox2/a.md", from: "Inbox", to: "Done") == nil)
    }

    @Test("Move to Trash puts the file in the macOS Trash, where it can be put back")
    func trash() throws {
        let folder = try TemporaryVault(files: ["Old \(UUID().uuidString).md": "keep me in the Trash"])
        let vault = Vault(root: folder.url)
        let name = try #require(try FileManager.default.contentsOfDirectory(atPath: folder.url.path).first)

        let inTrash = try vault.trash(name)
        defer { try? FileManager.default.removeItem(at: inTrash) }

        #expect(!FileManager.default.fileExists(atPath: folder.url.appending(path: name).path))
        #expect(try String(contentsOf: inTrash, encoding: .utf8) == "keep me in the Trash")
        #expect(inTrash.path.contains("/.Trash"))
    }
}
