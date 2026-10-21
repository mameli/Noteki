import Foundation
import Testing
import NotekiCore

@Suite("Open Note")
struct OpenNoteTests {
    @Test("the user's edits reach the file when saved")
    func savesEdits() throws {
        let folder = try TemporaryVault(files: ["Meetings/Retro.md": "# Retro\n\n## Personal notes\n"])
        let vault = Vault(root: folder.url)
        var open = try vault.open(Note(path: "Meetings/Retro.md"))

        open.edit("# Retro\n\n## Personal notes\n- ship on Friday\n")
        #expect(open.hasUnsavedEdits)
        try open.save()

        #expect(!open.hasUnsavedEdits)
        #expect(try folder.read("Meetings/Retro.md") == "# Retro\n\n## Personal notes\n- ship on Friday\n")
    }

    @Test("saving without edits writes nothing, so a change from another app is not overwritten")
    func saveWithoutEdits() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))

        try folder.write("# Retro\n\nSummary written by Takku\n", at: "Retro.md")
        open.edit("# Retro\n")
        try open.save()

        #expect(try folder.read("Retro.md") == "# Retro\n\nSummary written by Takku\n")
    }

    @Test("edits are not saved over a file another app changed: the edits stay, the file is left alone")
    func changedOnDisk() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))

        open.edit("# Retro\n\nmy notes\n")
        try folder.write("# Retro\n\nSummary written by Takku\n", at: "Retro.md")

        #expect(throws: OpenNote.SaveError.changedOnDisk) { try open.save() }
        #expect(open.hasUnsavedEdits)
        #expect(open.text == "# Retro\n\nmy notes\n")
        #expect(try folder.read("Retro.md") == "# Retro\n\nSummary written by Takku\n")
    }

    @Test("the user can choose to replace the other app's change with their own version")
    func replaceChangeOnDisk() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        open.edit("# Retro\n\nmy notes\n")
        try folder.write("# Retro\n\nSummary written by Takku\n", at: "Retro.md")

        try open.saveReplacingChangesOnDisk()
        open.edit("# Retro\n\nmy notes\nmore\n")
        try open.save()

        #expect(try folder.read("Retro.md") == "# Retro\n\nmy notes\nmore\n")
    }

    @Test("a Note without a final newline is saved without one")
    func noFinalNewline() throws {
        let folder = try TemporaryVault(files: ["Short.md": "one line"])
        var open = try Vault(root: folder.url).open(Note(path: "Short.md"))

        open.edit("one line, longer")
        try open.save()

        #expect(try folder.data("Short.md") == Data("one line, longer".utf8))
    }

    @Test("a Note with mixed line endings is saved with the ones most of its lines use")
    func mixedLineEndings() throws {
        let folder = try TemporaryVault(files: ["Mixed.md": "# Title\nfirst\nsecond\r\nthird\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Mixed.md"))

        open.edit("# Title\nfirst\nsecond\nthird\nfourth\n")
        try open.save()

        #expect(try folder.data("Mixed.md") == Data("# Title\nfirst\nsecond\nthird\nfourth\n".utf8))
    }

    @Test("a Note written with CRLF line endings keeps them, while the editor sees plain newlines")
    func keepsCRLF() throws {
        let folder = try TemporaryVault(files: ["Windows.md": "# Title\r\n\r\nfirst\r\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Windows.md"))

        #expect(open.text == "# Title\n\nfirst\n")
        open.edit("# Title\n\nfirst\nsecond\n")
        try open.save()

        #expect(try folder.data("Windows.md") == Data("# Title\r\n\r\nfirst\r\nsecond\r\n".utf8))
    }
}
