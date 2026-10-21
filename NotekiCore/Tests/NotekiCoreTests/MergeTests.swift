import Foundation
import Testing
import NotekiCore

@Suite("Merge and Conflict")
struct MergeTests {
    @Test("unsaved edits and an External change to other lines are merged, then saved")
    func merges() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n\nfirst\n\nlast\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        open.edit("# Retro\n\nfirst\n\nlast\nmine\n")

        try folder.write("# Retro\n\nfirst, by Takku\n\nlast\n", at: "Retro.md")
        let change = try open.noticeChanges(at: ["Retro.md"])

        guard case .reload(let merged) = change else {
            Issue.record("expected a merge, got \(change)")
            return
        }
        #expect(merged.text == "# Retro\n\nfirst, by Takku\n\nlast\nmine\n")
        let applied = open.apply(merged)
        #expect(applied)
        try open.save()
        #expect(try folder.read("Retro.md") == "# Retro\n\nfirst, by Takku\n\nlast\nmine\n")
    }

    @Test("edits to the same lines are a Conflict: the user's text stays, nothing is saved")
    func conflict() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\nline\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        open.edit("# Retro\nline, mine\n")

        try folder.write("# Retro\nline, Takku's\n", at: "Retro.md")

        #expect(try open.noticeChanges(at: ["Retro.md"]) == .conflict)
        #expect(open.conflict == .init(mine: "# Retro\nline, mine\n", theirs: "# Retro\nline, Takku's\n"))
        #expect(open.text == "# Retro\nline, mine\n")
        #expect(throws: OpenNote.SaveError.conflict) { try open.save() }
        #expect(try folder.read("Retro.md") == "# Retro\nline, Takku's\n")
    }

    @Test("a Conflict ends by itself when the other app puts the file back")
    func conflictUndone() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\nline\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        open.edit("# Retro\nline, mine\n")
        try folder.write("# Retro\nline, Takku's\n", at: "Retro.md")
        _ = try open.noticeChanges(at: ["Retro.md"])

        try folder.write("# Retro\nline\n", at: "Retro.md")
        _ = try open.noticeChanges(at: ["Retro.md"])

        #expect(open.conflict == nil)
        try open.save()
        #expect(try folder.read("Retro.md") == "# Retro\nline, mine\n")
    }

    @Test("blank lines on both sides do not confuse the Merge")
    func blankLines() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n\nfirst\n\nsecond\n\nthird\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        open.edit("# Retro\n\nfirst\n\nsecond\n\nthird\n\nmine\n")
        try folder.write("# Retro\n\nfirst, Takku's\n\n\nsecond\n\nthird\n", at: "Retro.md")

        guard case .reload(let merged) = try open.noticeChanges(at: ["Retro.md"]) else {
            Issue.record("expected a merge")
            return
        }
        #expect(merged.text == "# Retro\n\nfirst, Takku's\n\n\nsecond\n\nthird\n\nmine\n")
    }

    @Test("keep mine writes the user's text over the other app's")
    func keepMine() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\nline\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        open.edit("# Retro\nline, mine\n")
        try folder.write("# Retro\nline, Takku's\n", at: "Retro.md")
        _ = try open.noticeChanges(at: ["Retro.md"])

        try open.keepMine()

        #expect(open.conflict == nil)
        #expect(try folder.read("Retro.md") == "# Retro\nline, mine\n")
        open.edit("# Retro\nline, mine\nmore\n")
        try open.save()
        #expect(try folder.read("Retro.md") == "# Retro\nline, mine\nmore\n")
    }

    @Test("take theirs drops the user's edits for the file's text")
    func takeTheirs() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\nline\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        open.edit("# Retro\nline, mine\n")
        try folder.write("# Retro\nline, Takku's\n", at: "Retro.md")
        _ = try open.noticeChanges(at: ["Retro.md"])

        open.takeTheirs()

        #expect(open.conflict == nil)
        #expect(open.text == "# Retro\nline, Takku's\n")
        #expect(!open.hasUnsavedEdits)
    }

    @Test("a merge computed before the user typed again does not apply")
    func staleMerge() throws {
        let folder = try TemporaryVault(files: ["Retro.md": "# Retro\n\nfirst\n\nlast\n"])
        var open = try Vault(root: folder.url).open(Note(path: "Retro.md"))
        open.edit("# Retro\n\nfirst\n\nlast\nmine\n")
        try folder.write("# Retro\n\nfirst, by Takku\n\nlast\n", at: "Retro.md")
        guard case .reload(let merged) = try open.noticeChanges(at: ["Retro.md"]) else {
            Issue.record("expected a merge")
            return
        }

        open.edit("# Retro\n\nfirst\n\nlast\nmine, more\n")
        let applied = open.apply(merged)

        #expect(!applied)
        #expect(open.text == "# Retro\n\nfirst\n\nlast\nmine, more\n")
        guard case .reload(let again) = try open.noticeChanges(at: ["Retro.md"]) else {
            Issue.record("expected a merge on the second try")
            return
        }
        #expect(again.text == "# Retro\n\nfirst, by Takku\n\nlast\nmine, more\n")
    }

    @Test("Takku's Processing writes while the user keeps typing Personal notes: nothing is lost")
    func takkuProcessing() throws {
        let created = """
            ---
            takku_id: 6F1C2A00-0000-4000-8000-000000000001
            date: 2026-10-04T14:30
            tags: [meeting]
            ---
            %% takku:start %%
            ⏺ Recording in progress: the summary will appear here after you stop.
            %% takku:end %%

            ## Personal notes


            """
        let folder = try TemporaryVault(files: ["Meetings/2026-10-04 1430 - Meeting.md": created])
        let path = "Meetings/2026-10-04 1430 - Meeting.md"
        var open = try Vault(root: folder.url).open(Note(path: path))
        func takkuWrites(_ content: String) throws -> OpenNote.ExternalChange {
            try folder.write(content, at: path)
            return try open.noticeChanges(at: [path])
        }
        func applyMerge(_ change: OpenNote.ExternalChange) {
            guard case .reload(let merged) = change else {
                Issue.record("expected a merge, got \(change)")
                return
            }
            open.apply(merged)
        }

        // During the call: the user writes and saves, then keeps typing.
        open.edit(created + "- ask Mario about the budget\n")
        try open.save()
        open.edit(created + "- ask Mario about the budget\n- risk: supplier is late\n")

        // After the stop Takku writes the Summary, keeping the Personal notes it read.
        let saved = try folder.read(path)
        let processed = saved
            .replacingOccurrences(of: "tags: [meeting]\n", with: """
                tags: [meeting]
                duration: 47m
                transcript: "[[2026-10-04 1430 - Meeting (transcript)]]"
                language: it

                """)
            .replacingOccurrences(of: "⏺ Recording in progress: the summary will appear here after you stop.", with: """
                ### Budget
                - Approved.

                Full transcript: [[2026-10-04 1430 - Meeting (transcript)]]
                """)
        applyMerge(try takkuWrites(processed))
        open.edit(open.text + "- follow up on Friday\n")
        try open.save()

        #expect(try folder.read(path) == processed + "- risk: supplier is late\n- follow up on Friday\n")
    }
}
