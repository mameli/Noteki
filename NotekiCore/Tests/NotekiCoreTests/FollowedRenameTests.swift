import Foundation
import Testing
import NotekiCore

@Suite("Followed rename, replacement and deletion")
struct FollowedRenameTests {
    @Test("a Note renamed by another app stays open under its new name, and saves go there")
    func rename() throws {
        let folder = try TemporaryVault(files: ["Meetings/2026-10-04 1430 - Meeting.md": "# Meeting\n"])
        let vault = Vault(root: folder.url)
        var open = try vault.open(Note(path: "Meetings/2026-10-04 1430 - Meeting.md"))

        try folder.move("Meetings/2026-10-04 1430 - Meeting.md", to: "Meetings/2026-10-04 1430 - Retro.md")
        let change = try open.noticeChanges(at: [
            "Meetings/2026-10-04 1430 - Meeting.md", "Meetings/2026-10-04 1430 - Retro.md",
        ])

        #expect(change == .unchanged)
        #expect(open.note.path == "Meetings/2026-10-04 1430 - Retro.md")
        open.edit("# Meeting\nmine\n")
        try open.save()
        #expect(try folder.read("Meetings/2026-10-04 1430 - Retro.md") == "# Meeting\nmine\n")
        #expect(!FileManager.default.fileExists(atPath: folder.url.appending(path: "Meetings/2026-10-04 1430 - Meeting.md").path))
    }

    @Test("a Note replaced by a new file, as most editors save, is changed, not deleted")
    func replacement() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))

        try folder.replace("Retro.md", with: "# Retro\nfrom another editor\n")
        let change = try open.noticeChanges(at: ["Retro.md"])

        guard case .reload(let reload) = change else {
            Issue.record("expected a reload, got \(change)")
            return
        }
        let applied = open.apply(reload)
        #expect(applied)
        #expect(!open.isDeleted)
        // The new file is the one followed now.
        try folder.move("Retro.md", to: "Renamed.md")
        _ = try open.noticeChanges(at: ["Retro.md", "Renamed.md"])
        #expect(open.note.path == "Renamed.md")
    }

    @Test("Takku writes the Summary in place, then renames the note: both are followed")
    func takkuRename() throws {
        let path = "Meetings/2026-10-04 1430 - Meeting.md"
        let folder = try TemporaryVault(files: [path: "# Meeting\n"])
        var open = try Vault(root: folder.url).open(Note(path: path))

        let handle = try FileHandle(forWritingTo: folder.url.appending(path: path))
        try handle.write(contentsOf: Data("# Meeting\n\n### Budget\n".utf8))
        try handle.close()
        try folder.move(path, to: "Meetings/2026-10-04 1430 - Budget review.md")
        let change = try open.noticeChanges(at: [path, "Meetings/2026-10-04 1430 - Budget review.md"])

        #expect(open.note.path == "Meetings/2026-10-04 1430 - Budget review.md")
        guard case .reload(let reload) = change else {
            Issue.record("expected a reload, got \(change)")
            return
        }
        #expect(reload.text == "# Meeting\n\n### Budget\n")
    }

    @Test("a Note deleted by another app keeps its text and is saved only when the user says so")
    func deletion() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        open.edit("# Retro\nmine\n")

        try folder.delete("Retro.md")

        #expect(try open.noticeChanges(at: ["Retro.md"]) == .deleted)
        #expect(open.isDeleted)
        #expect(open.text == "# Retro\nmine\n")
        #expect(throws: OpenNote.SaveError.deleted) { try open.save() }
        #expect(!FileManager.default.fileExists(atPath: folder.url.appending(path: "Retro.md").path))

        try open.saveAgain()

        #expect(!open.isDeleted)
        #expect(try folder.read("Retro.md") == "# Retro\nmine\n")
    }

    @Test("a rename reported in two parts is still followed")
    func renameInTwoParts() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))

        try folder.move("Retro.md", to: "Renamed.md")

        #expect(try open.noticeChanges(at: ["Retro.md"]) == .deleted)
        #expect(try open.noticeChanges(at: ["Renamed.md"]) == .unchanged)
        #expect(open.note.path == "Renamed.md")
        #expect(!open.isDeleted)
    }

    @Test("a deleted Note written again at the same path by another app is followed again")
    func recreated() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        try folder.delete("Retro.md")
        _ = try open.noticeChanges(at: ["Retro.md"])

        try folder.write("# Retro\nback\n", at: "Retro.md")
        let change = try open.noticeChanges(at: ["Retro.md"])

        #expect(!open.isDeleted)
        guard case .reload(let reload) = change else {
            Issue.record("expected a reload, got \(change)")
            return
        }
        #expect(reload.text == "# Retro\nback\n")
    }

    @Test("rewritten as a new file and renamed in the same moment, the Note is recognised by its text")
    func atomicWriteThenRename() throws {
        let folder = try TemporaryVault(files: [
            "Meetings/Meeting.md": "# Meeting\n\n## Personal notes\n\n- ask Mario\n",
            "Meetings/Other.md": "# Something else entirely\n",
        ])
        var open = try Vault(root: folder.url).open(Note(path: "Meetings/Meeting.md"))

        try folder.replace("Meetings/Meeting.md", with: "# Meeting\n\n### Budget\n\n## Personal notes\n\n- ask Mario\n")
        try folder.move("Meetings/Meeting.md", to: "Meetings/Budget.md")
        try folder.write("# Something else entirely\nchanged too\n", at: "Meetings/Other.md")
        let change = try open.noticeChanges(at: ["Meetings/Meeting.md", "Meetings/Budget.md", "Meetings/Other.md"])

        #expect(open.note.path == "Meetings/Budget.md")
        #expect(!open.isDeleted)
        guard case .reload = change else {
            Issue.record("expected a reload, got \(change)")
            return
        }
    }

    @Test("a rename while the user has unsaved edits keeps them, and they are saved under the new name")
    func renameWithUnsavedEdits() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        open.edit("# Retro\nmine\n")

        try folder.move("Retro.md", to: "Renamed.md")
        #expect(throws: OpenNote.SaveError.missing) { try open.save() }
        _ = try open.noticeChanges(at: ["Retro.md", "Renamed.md"])

        #expect(open.text == "# Retro\nmine\n")
        try open.save()
        #expect(try folder.read("Renamed.md") == "# Retro\nmine\n")
    }

    @Test("saving a deleted Note again ends a Conflict it had")
    func saveAgainEndsConflict() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\nline\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        open.edit("# Retro\nline, mine\n")
        try folder.write("# Retro\nline, Takku's\n", at: "Retro.md")
        _ = try open.noticeChanges(at: ["Retro.md"])
        try folder.delete("Retro.md")
        _ = try open.noticeChanges(at: ["Retro.md"])

        try open.saveAgain()

        #expect(open.conflict == nil)
        #expect(try folder.read("Retro.md") == "# Retro\nline, mine\n")
    }

    @Test("another file renamed onto the Note's path is not the Note: the Note is followed where it went")
    func swap() throws {
        let folder = try TemporaryVault(files: ["A.md": "# A\nthe Note\n", "B.md": "# B\nanother\n"])
        var open = try Vault(root: folder.url).open(Note(path: "A.md"))
        open.edit("# A\nthe Note\nmine\n")

        try folder.move("A.md", to: "C.md")
        try folder.move("B.md", to: "A.md")
        _ = try open.noticeChanges(at: ["A.md", "B.md", "C.md"])

        #expect(open.note.path == "C.md")
        #expect(open.text == "# A\nthe Note\nmine\n")
        try open.save()
        #expect(try folder.read("C.md") == "# A\nthe Note\nmine\n")
        #expect(try folder.read("A.md") == "# B\nanother\n")
    }

    @Test("a replacement with the same bytes is still followed when renamed later")
    func sameBytesReplacement() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))

        try folder.replace("Retro.md", with: "# Retro\n")
        #expect(try open.noticeChanges(at: ["Retro.md"]) == .unchanged)
        try folder.move("Retro.md", to: "Renamed.md")
        _ = try open.noticeChanges(at: ["Retro.md", "Renamed.md"])

        #expect(open.note.path == "Renamed.md")
        #expect(!open.isDeleted)
    }

    @Test("a rename that only changes letter case is followed, and saves keep the new name")
    func caseOnlyRename() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))

        try folder.move("Retro.md", to: "retro.md")
        // The file system reports the name as it is on disk now.
        _ = try open.noticeChanges(at: ["retro.md"])

        #expect(open.note.path == "retro.md")
        open.edit("# Retro\nmine\n")
        try open.save()
        #expect(try FileManager.default.contentsOfDirectory(atPath: folder.url.path).contains("retro.md"))
    }

    @Test("a deleted Note forgets a Conflict it had: the other version is gone too")
    func deletionEndsConflict() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\nline\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        open.edit("# Retro\nline, mine\n")
        try folder.write("# Retro\nline, Takku's\n", at: "Retro.md")
        _ = try open.noticeChanges(at: ["Retro.md"])

        try folder.delete("Retro.md")
        _ = try open.noticeChanges(at: ["Retro.md"])

        #expect(open.conflict == nil)
        #expect(open.isDeleted)
    }

    @Test("a text file opened on its own is followed when renamed, like a Note")
    func textFileRename() throws {
        let folder = try TemporaryVault(files: ["todo.txt": "buy milk\n"])
        var open = try Vault(root: folder.url).open(Note(path: "todo.txt"))

        try folder.move("todo.txt", to: "groceries.txt")
        _ = try open.noticeChanges(at: ["todo.txt", "groceries.txt"])

        #expect(open.note.path == "groceries.txt")
        #expect(!open.isDeleted)
    }
}
