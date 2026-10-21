import AppKit
import NotekiCore
import Observation
import os

private let logger = Logger(subsystem: "app.noteki.noteki", category: "vault")

/// The open Vault and the Note shown. The Vault's location is remembered in the app's
/// preferences, outside the Vault (ADR 0002).
@MainActor
@Observable
final class AppModel {
    private static let vaultPathKey = "vaultPath"

    private(set) var vault: Vault?
    private(set) var tree: [VaultItem] = []
    private(set) var note: Note?
    private(set) var noteText = ""
    /// Why the Vault or the Note cannot be shown, in words for the user.
    private(set) var failure: String?

    /// The path of the row selected in the Files pane: a Note or a folder.
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
        note = nil
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
        guard let vault, let selection, case .note(let note)? = tree.item(at: selection) else { return }
        do {
            noteText = try vault.text(of: note)
            self.note = note
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
