import AppKit
import NotekiCore
import Observation
import os

private let logger = Logger(subsystem: "app.noteki.noteki", category: "vault")

/// The open Vault and what it shows: a Note or an Attachment. The Vault's location is
/// remembered in the app's preferences, outside the Vault (ADR 0002).
@MainActor
@Observable
final class AppModel {
    private static let vaultPathKey = "vaultPath"

    private(set) var vault: Vault?
    private(set) var tree: [VaultItem] = []
    /// The Note or Attachment selected, nil before the first choice.
    private(set) var shown: Shown?
    /// The text of the last Note read: the editor keeps it while an Attachment is in front.
    private(set) var noteText = ""
    /// Why the Vault or what is shown cannot be read, in words for the user.
    private(set) var failure: String?

    enum Shown: Equatable {
        case note(Note)
        /// In a native view, in front of the editor.
        case attachment(Attachment)

        var name: String {
            switch self {
            case .note(let note): note.name
            case .attachment(let attachment): attachment.name
            }
        }
    }

    /// The path of the row selected in the Files pane: a Note, an Attachment or a folder.
    var selection: String? {
        didSet { if selection != oldValue { showSelection() } }
    }

    init() {
        guard let path = UserDefaults.standard.string(forKey: Self.vaultPathKey) else { return }
        var isFolder: ObjCBool = false
        if FileManager.default.fileExists(atPath: path, isDirectory: &isFolder), isFolder.boolValue {
            open(URL(filePath: path, directoryHint: .isDirectory))
        } else {
            // Moved or deleted since last time: ask again, as at first launch.
            logger.info("The remembered Vault is gone, asking for another")
        }
    }

    func chooseVault() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.prompt = "Open Vault"
        panel.message = Self.chooseVaultMessage
        guard panel.runModal() == .OK, let url = panel.url else { return }
        open(url)
    }

    static let chooseVaultMessage = "Choose the Vault: the folder of your notes. It can be an Obsidian vault."

    func open(_ folder: URL) {
        let vault = Vault(root: folder)
        self.vault = vault
        tree = []
        shown = nil
        noteText = ""
        selection = nil
        failure = nil
        UserDefaults.standard.set(vault.root.path(percentEncoded: false), forKey: Self.vaultPathKey)
        Task {
            // Off the main thread: a large Vault takes a moment to list.
            let result = await Task.detached { Result { try vault.tree() } }.value
            guard self.vault?.root == vault.root else { return }
            switch result {
            case .success(let tree):
                self.tree = tree
            case .failure(let error):
                logger.error("Cannot list the Vault: \(error.localizedDescription, privacy: .public)")
                failure = "Cannot read the Vault \(vault.root.lastPathComponent): \(error.localizedDescription)"
            }
        }
    }

    private func showSelection() {
        guard let vault, let selection else { return }
        switch tree.item(at: selection) {
        case .note(let note): show(note, in: vault)
        case .attachment(let attachment):
            shown = .attachment(attachment)
            failure = nil
        case .folder, nil: break
        }
    }

    private func show(_ note: Note, in vault: Vault) {
        shown = .note(note)
        do {
            noteText = try vault.text(of: note)
            failure = nil
        } catch {
            logger.error("Cannot read a Note: \(error.localizedDescription, privacy: .public)")
            failure = "Cannot read \(note.name): \(error.localizedDescription)"
        }
    }
}

private extension [VaultItem] {
    func item(at path: String) -> VaultItem? {
        for item in self {
            if item.path == path { return item }
            if case .folder(let folder, let children) = item, path.hasPrefix(folder + "/") {
                return children.item(at: path)
            }
        }
        return nil
    }
}
