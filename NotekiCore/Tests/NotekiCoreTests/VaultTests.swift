import Foundation
import Testing
import NotekiCore

@Suite("Vault")
struct VaultTests {
    @Test("lists folders and Notes as a tree, folders first, in Finder order")
    func tree() throws {
        let folder = try TemporaryVault(files: [
            "Note 10.md": "",
            "Note 2.md": "",
            "b.md": "",
            "Meetings/2026-10-04 1430 - Retro.md": "",
            "Meetings/Transcripts/2026-10-04 1430 - Retro (transcript).md": "",
            "Agile/Unipol/Analisi.md": "",
        ])

        let vault = Vault(root: folder.url)

        #expect(try outline(vault.tree()) == """
            Agile/
              Unipol/
                Analisi
            Meetings/
              Transcripts/
                2026-10-04 1430 - Retro (transcript)
              2026-10-04 1430 - Retro
            b
            Note 2
            Note 10
            """)
    }

    @Test("lists only Markdown files as Notes")
    func onlyMarkdown() throws {
        let folder = try TemporaryVault(files: [
            "Note.md": "",
            "script.py": "",
            "data.json": "",
            "README.markdown": "",
        ])

        #expect(try outline(Vault(root: folder.url).tree()) == "Note")
    }

    @Test("reads the text of a Note as written")
    func text() throws {
        let content = "---\ntags: [meeting]\n---\n# Retro\r\n\r\nàèìòù [[Other]]\n"
        let folder = try TemporaryVault(files: ["Meetings/Retro.md": content])
        let vault = Vault(root: folder.url)

        let note = try #require(vault.tree().notes.first)

        #expect(note.path == "Meetings/Retro.md")
        #expect(note.name == "Retro")
        #expect(try vault.text(of: note) == content)
    }

    @Test("opening a Vault writes nothing in it")
    func writesNothing() throws {
        let folder = try TemporaryVault(files: ["Note.md": "text", ".obsidian/app.json": "{}"])
        let before = try folder.snapshot()

        let vault = Vault(root: folder.url)
        _ = try vault.tree()
        _ = try vault.text(of: #require(vault.tree().notes.first))

        #expect(try folder.snapshot() == before)
    }
}

/// The tree as indented lines: folders end with `/`, Notes are shown by name.
private func outline(_ items: [VaultItem], depth: Int = 0) -> String {
    items.map { item in
        let indent = String(repeating: "  ", count: depth)
        switch item {
        case .folder(_, let children):
            let lines = indent + item.name + "/"
            return children.isEmpty ? lines : lines + "\n" + outline(children, depth: depth + 1)
        case .note(let note):
            return indent + note.name
        }
    }.joined(separator: "\n")
}

extension [VaultItem] {
    var notes: [Note] {
        flatMap { item -> [Note] in
            switch item {
            case .folder(_, let children): children.notes
            case .note(let note): [note]
            }
        }
    }
}
