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

@Suite("External change")
struct ExternalChangeTests {
    @Test("a Note changed by another app, with nothing unsaved, is reloaded once the editor shows it")
    func reloads() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n"])
        let vault = Vault(root: folder.url)
        var open = try vault.open(Note(path: "Retro.md"))

        try folder.write("# Retro\n\nSummary written by Takku\n", at: "Retro.md")
        let change = try open.noticeChanges(at: vault.changedPaths([folder.url.appending(path: "Retro.md")]))

        guard case .reload(let reload) = change else {
            Issue.record("expected a reload, got \(change)")
            return
        }
        #expect(reload.text == "# Retro\n\nSummary written by Takku\n")
        open.apply(reload)
        #expect(open.text == "# Retro\n\nSummary written by Takku\n")
        open.edit("# Retro\n\nSummary written by Takku\n\nmy notes\n")
        try open.save()
        #expect(try folder.read("Retro.md") == "# Retro\n\nSummary written by Takku\n\nmy notes\n")
    }

    @Test("a reload the editor could not show changes nothing: the user's next save is refused")
    func reloadNotShown() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n"])
        let vault = Vault(root: folder.url)
        var open = try vault.open(Note(path: "Retro.md"))

        try folder.write("# Retro\nTakku\n", at: "Retro.md")
        _ = try open.noticeChanges(at: ["Retro.md"])
        // The user typed before the reload reached the editor: their text, on the old one.
        open.edit("# Retro\nmine\n")

        #expect(throws: OpenNote.SaveError.changedOnDisk) { try open.save() }
        #expect(try folder.read("Retro.md") == "# Retro\nTakku\n")
    }

    @Test("changes to other files leave the open Note alone")
    func otherFiles() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n", "Other.md": ""])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))

        try folder.write("# Retro\nTakku\n", at: "Retro.md")

        #expect(try open.noticeChanges(at: ["Other.md"]) == .unchanged)
    }

    @Test("the event of Noteki's own save changes nothing")
    func ownSave() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        open.edit("# Retro\nmine\n")
        try open.save()

        #expect(try open.noticeChanges(at: ["Retro.md"]) == .unchanged)
        #expect(open.text == "# Retro\nmine\n")
    }
}
