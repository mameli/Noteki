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

    @Test("hidden folders are Excluded folders")
    func hiddenFolders() throws {
        let folder = try TemporaryVault(files: [
            "Note.md": "",
            ".obsidian/workspace.md": "",
            ".trash/Old.md": "",
            "Meetings/.cache/x.md": "",
            "Meetings/Retro.md": "",
        ])

        #expect(try outline(Vault(root: folder.url).tree()) == """
            Meetings/
              Retro
            Note
            """)
    }

    @Test("dependency and environment folders of code are Excluded folders, at any depth")
    func codeFolders() throws {
        let folder = try TemporaryVault(files: [
            "node_modules/pkg/README.md": "",
            "Projects/playground/node_modules/pkg/README.md": "",
            "Projects/playground/venv/lib/LICENSE.md": "",
            "Projects/playground/.venv/lib/LICENSE.md": "",
            "Projects/playground/__pycache__/x.md": "",
            "Projects/playground/Notes.md": "",
        ])

        #expect(try outline(Vault(root: folder.url).tree()) == """
            Projects/
              playground/
                Notes
            """)
    }

    @Test("a repository inside the Vault is an Excluded folder, the Vault's own repository is not")
    func repositories() throws {
        let folder = try TemporaryVault(files: [
            ".git/HEAD": "ref: refs/heads/main",
            "Note.md": "",
            "Projects/refactoring-agent/.git/HEAD": "ref: refs/heads/main",
            "Projects/refactoring-agent/README.md": "",
            "Projects/submodule/.git": "gitdir: ../../.git/modules/submodule",
            "Projects/submodule/README.md": "",
            "Projects/Course.md": "",
        ])

        #expect(try outline(Vault(root: folder.url).tree()) == """
            Projects/
              Course
            Note
            """)
    }

    @Test("a folder that cannot be read is left out, the rest of the Vault is listed")
    func unreadableFolder() throws {
        let folder = try TemporaryVault(files: ["Private/Secret.md": "", "Note.md": ""])
        let locked = folder.url.appending(path: "Private").path(percentEncoded: false)
        try FileManager.default.setAttributes([.posixPermissions: 0o000], ofItemAtPath: locked)
        defer { try? FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: locked) }

        #expect(try outline(Vault(root: folder.url).tree()) == "Note")
    }

    @Test("lists images and PDFs as Attachments among the Notes, in Finder order")
    func attachments() throws {
        let folder = try TemporaryVault(files: [
            "Meetings/Retro.md": "",
            "Meetings/Pasted image 20260917102056.png": "",
            "Meetings/Screenshots/Slide.jpeg": "",
            "Contract.pdf": "",
            "Diagram.svg": "",
            "IMG_0042.HEIC": "",
            "Photo.JPG": "",
            "anim.gif": "",
            "banner.webp": "",
            "notes.txt": "",
            "page.html": "",
        ])

        #expect(try outline(Vault(root: folder.url).tree()) == """
            Meetings/
              Screenshots/
                [image] Slide.jpeg
              [image] Pasted image 20260917102056.png
              Retro
            [image] anim.gif
            [image] banner.webp
            [pdf] Contract.pdf
            [image] Diagram.svg
            [image] IMG_0042.HEIC
            [image] Photo.JPG
            """)
    }

    @Test("a file system event becomes a path in the Vault, unless it is outside it or in an Excluded folder")
    func changePaths() throws {
        let folder = try TemporaryVault(files: [
            "Meetings/Retro.md": "",
            ".git/index": "",
            "Code/repo/.git/HEAD": "",
            "Code/repo/README.md": "",
            "Code/node_modules/x/a.md": "",
        ])
        let vault = Vault(root: folder.url)
        // FSEvents reports real paths: /private/var/… for a Vault opened as /var/….
        let events = folder.url.resolvingSymlinksInPath()

        #expect(vault.changedPaths([
            events.appending(path: "Meetings/Retro.md"),
            folder.url.appending(path: "Meetings/New.md"),
            events.appending(path: ".git/index"),
            events.appending(path: "Code/repo/README.md"),
            events.appending(path: "Code/node_modules/x/a.md"),
            URL(filePath: "/tmp/elsewhere.md"),
        ]) == ["Meetings/Retro.md", "Meetings/New.md"])
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

/// The tree as indented lines: folders end with `/`, Notes are shown by name, Attachments by
/// file name after their kind.
private func outline(_ items: [VaultItem], depth: Int = 0) -> String {
    items.map { item in
        let indent = String(repeating: "  ", count: depth)
        switch item {
        case .folder(_, let children):
            let lines = indent + item.name + "/"
            return children.isEmpty ? lines : lines + "\n" + outline(children, depth: depth + 1)
        case .note(let note):
            return indent + note.name
        case .attachment(let attachment):
            return indent + "[\(attachment.kind)] " + attachment.name
        }
    }.joined(separator: "\n")
}

extension [VaultItem] {
    var notes: [Note] {
        flatMap { item -> [Note] in
            switch item {
            case .folder(_, let children): children.notes
            case .note(let note): [note]
            case .attachment: []
            }
        }
    }
}
