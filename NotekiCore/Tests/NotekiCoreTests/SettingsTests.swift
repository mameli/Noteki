import Foundation
import Testing
import NotekiCore

@Suite("Settings")
struct SettingsTests {
    @Test("a folder the user excludes, by its path in any letter case, is neither listed nor indexed, with what it holds")
    func userExcludedFolder() throws {
        let folder = try TemporaryVault(files: [
            "Note.md": "",
            "Archive/Old.md": "",
            "Clients/Axo/Plan.md": "",
            "Clients/Axo/Drafts/x.md": "",
            "Clients/Unipol/Plan.md": "",
            "Archive 2/Kept.md": "",
        ])
        let vault = Vault(root: folder.url, excludedFolders: ["archive", "/Clients/Axo/"])

        let names = VaultIndex(tree: try vault.tree()).notes.map(\.path)

        #expect(names.sorted() == ["Archive 2/Kept.md", "Clients/Unipol/Plan.md", "Note.md"])
    }

    @Test("a change, an opening or a new Note in a folder the user excludes belongs to it, not to the Vault")
    func userExcludedElsewhere() throws {
        let folder = try TemporaryVault(files: ["Note.md": "", "Archive/Old.md": ""])
        let vault = Vault(root: folder.url, excludedFolders: ["Archive", "Private"])
        let empty = try vault.newFolder(in: "")

        #expect(vault.changedPaths([folder.url.appending(path: "Archive/Old.md")]).isEmpty)
        #expect(vault.place(folder.url.appending(path: "Archive/Old.md")) == .excluded)
        #expect(throws: Vault.CreateError.outside) { try vault.createNote(for: "Archive/New", from: nil) }
        #expect(throws: Vault.FileError.invalidName) { try vault.rename(empty, to: "Private") }
    }

    @Test("the editor's settings: the system font at 15 points and 46 rem wide unless chosen, within bounds")
    func editorSettings() throws {
        #expect(EditorSettings.standard == EditorSettings(font: nil, size: 15, lineWidth: 736))
        #expect(EditorSettings(font: "  ", size: 3, lineWidth: 99_999) == EditorSettings(font: nil, size: 10, lineWidth: 1600))
        #expect(EditorSettings(font: "Iowan Old Style", size: 18.4, lineWidth: 400).size == 18)
        #expect(EditorSettings.standard.with(font: "Menlo").with(size: 40) == EditorSettings(font: "Menlo", size: 32, lineWidth: 736))
        #expect(EditorSettings(font: "Menlo", size: 15, lineWidth: 736).with(font: .some(nil)) == .standard)
    }

    @Test("the editor gets its settings as JSON, the system font as none")
    func editorSettingsJSON() throws {
        let command = EditorCommand.settings(EditorSettings(font: "Iowan Old Style", size: 17, lineWidth: 800))

        #expect(try command.json() == #"{"settings":{"font":"Iowan Old Style","lineWidth":800,"size":17},"type":"settings"}"#)
        #expect(try EditorSettings.standard.json() == #"{"lineWidth":736,"size":15}"#)
    }
}
