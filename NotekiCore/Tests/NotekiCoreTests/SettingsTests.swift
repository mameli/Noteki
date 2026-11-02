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

    @Test("the editor's settings: the system font at 15 points and lines of 100 characters unless chosen, within bounds")
    func editorSettings() throws {
        #expect(EditorSettings.standard == EditorSettings(font: nil, size: 15, lineLength: 100))
        #expect(EditorSettings(font: "  ", size: 3, lineLength: 99_999) == EditorSettings(font: nil, size: 10, lineLength: 200))
        #expect(EditorSettings(font: "Iowan Old Style", size: 18.4, lineLength: 10).size == 18)
        #expect(EditorSettings(font: nil, size: 15, lineLength: 72.4).lineLength == 72)
        #expect(EditorSettings.standard.with(font: "Menlo").with(size: 40) == EditorSettings(font: "Menlo", size: 32, lineLength: 100))
        #expect(EditorSettings(font: "Menlo", size: 15, lineLength: 100).with(font: .some(nil)) == .standard)
        #expect(EditorSettings.standard.with(monoFont: " ").monoFont == nil)
        #expect(EditorSettings.standard.with(limitsLineLength: false).lineLength == 100)
        #expect(EditorSettings.standard.theme == .ayu)
        #expect(EditorSettings.standard.with(theme: .oneDarkPro).theme.rawValue == "one-dark-pro")
    }

    @Test("a width in points kept until 0.1.0 is read as the length that draws it as wide at its size")
    func lineWidthBecomesLength() {
        #expect(EditorSettings.lineLength(width: 736, size: 15).rounded() == 98)
        #expect(EditorSettings.lineLength(width: 800, size: 20) == 80)
    }

    @Test("the editor gets its settings as JSON, the system font as none")
    func editorSettingsJSON() throws {
        let command = EditorCommand.settings(EditorSettings(
            theme: .everforest, font: "Iowan Old Style", monoFont: "JetBrains Mono", size: 17, lineLength: 80, limitsLineLength: false
        ))

        #expect(try command.json() == #"{"settings":{"font":"Iowan Old Style","limitsLineLength":false,"lineLength":80,"monoFont":"JetBrains Mono","size":17,"theme":"everforest"},"type":"settings"}"#)
        #expect(try EditorSettings.standard.json() == #"{"limitsLineLength":true,"lineLength":100,"size":15,"theme":"ayu"}"#)
    }
}
