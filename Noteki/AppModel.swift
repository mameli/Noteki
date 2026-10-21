import AppKit
import NotekiCore
import Observation
import os

private let logger = Logger(subsystem: "app.noteki.noteki", category: "vault")

/// The open Vault and what it shows: a Note or an Attachment. The Vault's location is
/// remembered in the app's preferences, outside the Vault (ADR 0002). The Note shown is open
/// for writing and saved by itself about a second after the last change.
@MainActor
@Observable
final class AppModel {
    private static let vaultPathKey = "vaultPath"

    private(set) var vault: Vault?
    private(set) var tree: [VaultItem] = []
    /// The Note or Attachment selected, nil before the first choice.
    private(set) var shown: Shown? {
        didSet { editor.isInFront = isShowingNote }
    }
    /// Why the Vault or what is shown cannot be read, in words for the user.
    private(set) var failure: String? {
        didSet { editor.isInFront = isShowingNote }
    }
    /// Why the last save failed; cleared by the next one that works.
    private(set) var saveFailure: String?

    /// The window's editor: one web view per window (ADR 0003), one window until ticket 25.
    let editor = EditorController()
    /// The Note in the editor, kept while an Attachment is in front.
    private var openNote: OpenNote? {
        didSet { conflict = openNote.flatMap { open in open.conflict.map { .init(mine: open.text, theirs: $0.theirs) } } }
    }
    /// The open Note's Conflict, for the banner: the user's text as it is now and the file's.
    private(set) var conflict: OpenNote.Conflict?
    /// The Note just left: keys typed in the instant before the switch still belong to it.
    private var previousNote: OpenNote?
    /// What the editor tags its reports with: the open Note for as long as it stays open, under
    /// whatever name another app gives it. Not its path, which a Followed rename changes.
    private var openNoteID = ""
    private var previousNoteID = ""
    /// The last revision applied per Note: a report that arrives after a later one is stale.
    private var revisions: [String: Int] = [:]
    private var pendingSave: Task<Void, Never>?
    private var watcher: VaultWatcher?
    private var pendingTreeRefresh: Task<Void, Never>?
    /// Switching Note, Vault or quitting waits for the previous switch: each one saves first.
    private var transition: Task<Void, Never>?
    private static let saveDelay: Duration = .seconds(1)

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
        didSet {
            guard selection != oldValue, let selection else { return }
            afterTransition { await self.show(selection) }
        }
    }

    init() {
        editor.onChange = { [weak self] noteID, revision, text in
            self?.edited(noteID: noteID, revision: revision, text: text)
        }
        guard let path = UserDefaults.standard.string(forKey: Self.vaultPathKey) else { return }
        var isFolder: ObjCBool = false
        if FileManager.default.fileExists(atPath: path, isDirectory: &isFolder), isFolder.boolValue {
            // At once: nothing is open yet to save, and the window must not ask for a Vault.
            activate(Vault(root: URL(filePath: path, directoryHint: .isDirectory)))
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

    /// A Note is in front, not an Attachment or a placeholder: the editor takes commands.
    var isShowingNote: Bool {
        if case .note? = shown, failure == nil { true } else { false }
    }

    func open(_ folder: URL) {
        afterTransition { await self.switchVault(to: folder) }
    }

    /// Saves what the user typed before the app quits. False when edits could not be saved and
    /// the user chose to stay.
    func prepareToQuit() async -> Bool {
        await transition?.value
        return await leaveOpenNote()
    }

    private func switchVault(to folder: URL) async {
        guard await leaveOpenNote() else { return }
        activate(Vault(root: folder))
    }

    private func activate(_ vault: Vault) {
        self.vault = vault
        watcher = VaultWatcher(root: vault.root) { [weak self] change in self?.filesChanged(change) }
        tree = []
        shown = nil
        openNote = nil
        previousNote = nil
        revisions = [:]
        editor.clear()
        selection = nil
        failure = nil
        UserDefaults.standard.set(vault.root.path(percentEncoded: false), forKey: Self.vaultPathKey)
        Task { await listTree(of: vault) }
    }

    /// Lists Files. `refresh` after a change on disk: a failure then keeps the last list and
    /// leaves the Note on screen, it is only logged.
    private func listTree(of vault: Vault, refresh: Bool = false) async {
        // Off the main thread: a large Vault takes a moment to list.
        let result = await Task.detached { Result { try vault.tree() } }.value
        guard self.vault?.root == vault.root else { return }
        switch result {
        case .success(let tree):
            self.tree = tree
        case .failure(let error):
            logger.error("Cannot list the Vault: \(error.localizedDescription, privacy: .public)")
            guard !refresh else { return }
            failure = "Cannot read the Vault \(vault.root.lastPathComponent): \(error.localizedDescription)"
        }
    }

    /// From the Vault's watcher: files changed on disk, by Noteki or another app.
    private func filesChanged(_ change: VaultWatcher.Change) {
        guard let vault else { return }
        var paths = vault.changedPaths(change.urls)
        // Events were lost or merged into folders: anything may have changed.
        if change.mustRescan, let open = openNote { paths.insert(open.note.path) }
        guard !paths.isEmpty else { return }
        if change.mustRescan || paths.contains(where: { appearedOrDisappeared($0, in: vault) }) {
            // A Note or Attachment created or deleted by another app, such as Takku's new
            // Meeting notes: Files lists it without a relaunch.
            pendingTreeRefresh?.cancel()
            pendingTreeRefresh = Task {
                try? await Task.sleep(for: .milliseconds(300))
                guard !Task.isCancelled else { return }
                await listTree(of: vault, refresh: true)
            }
        }
        // Case-insensitively: a rename that only changes letter case is reported under the new name.
        if let open = openNote,
           open.isDeleted || paths.contains(where: { $0.caseInsensitiveCompare(open.note.path) == .orderedSame }) {
            afterTransition { await self.noticeChanges(at: paths) }
        }
    }

    /// Whether the file is new to Files or gone from disk; a save to a listed file is neither.
    private func appearedOrDisappeared(_ path: String, in vault: Vault) -> Bool {
        let exists = FileManager.default.fileExists(atPath: vault.root.appending(path: path).path(percentEncoded: false))
        return exists != (tree.item(at: path) != nil)
    }

    /// The open Note may have changed on disk (ADR 0005): reloaded when nothing is unsaved,
    /// merged with the user's edits and saved, or a Conflict left to the user.
    private func noticeChanges(at paths: Set<String>) async {
        // If the user types while a merge is on its way to the editor, it is computed again.
        for _ in 1...3 {
            // Keys the editor has not reported yet are unsaved edits too.
            await editor.flush()
            guard var open = openNote else { return }
            let change: OpenNote.ExternalChange
            do {
                change = try open.noticeChanges(at: paths)
            } catch {
                logger.error("Cannot read a Note changed on disk: \(error.localizedDescription, privacy: .public)")
                return
            }
            let before = openNote?.note
            openNote = open
            if let before, open.note != before, shown == .note(before) {
                // A Followed rename of the Note in front: the window shows its new name and Files
                // selects it. Behind an Attachment, or once the user chose another row, nothing moves.
                shown = .note(open.note)
                if selection == before.path { selection = open.note.path }
            }
            switch change {
            case .unchanged:
                // A Followed rename may have stopped a save at the old name: it goes to the new one.
                if open.hasUnsavedEdits, !open.isDeleted, open.conflict == nil {
                    saveFailure = nil
                    scheduleSave()
                }
                return
            case .deleted:
                pendingSave?.cancel()
                saveFailure = nil
                logger.info("The open Note was deleted by another app")
                return
            case .conflict:
                pendingSave?.cancel()
                saveFailure = nil
                logger.info("Conflict with a change on disk")
                return
            case .reload(let reload):
                let shown = await editor.reload(
                    noteID: openNoteID, revision: revisions[openNoteID, default: 0], text: reload.text
                )
                // Only what the editor shows becomes the base, and only if the user's text is
                // still the one the merge started from.
                guard shown, var current = openNote, current.note == open.note, current.apply(reload) else {
                    continue
                }
                openNote = current
                saveFailure = nil
                logger.info("Reloaded a Note changed on disk")
                // A merge holds the user's edits: saved at once, over the file it came from.
                if current.hasUnsavedEdits { await saveNow() }
                return
            }
        }
        logger.error("A change on disk could not reach the editor, the next save tries again")
    }

    /// The open Note another app deleted: true while its text is only on screen.
    var isOpenNoteDeleted: Bool { openNote?.isDeleted == true }

    /// Writes the deleted Note again where it was, with the text on screen.
    func saveAgain() {
        afterTransition {
            await self.editor.flush()
            guard var open = self.openNote, open.isDeleted else { return }
            do {
                try open.saveAgain()
                self.saveFailure = nil
            } catch {
                self.saveFailure = "Cannot save \(open.note.name): \(error.localizedDescription)"
            }
            self.openNote = open
        }
    }

    /// Ends the Conflict with the user's version, written over the other app's change.
    func keepMine() {
        afterTransition {
            await self.editor.flush()
            guard var open = self.openNote, open.conflict != nil else { return }
            do {
                try open.keepMine()
                self.saveFailure = nil
            } catch {
                self.saveFailure = "Cannot save \(open.note.name): \(error.localizedDescription)"
            }
            self.openNote = open
        }
    }

    /// Ends the Conflict with the other app's version: the user's edits are dropped. Shown as a
    /// reload, so the cursor stays where it was.
    func takeTheirs() {
        afterTransition {
            for _ in 1...3 {
                // Keys typed just before the click arrive now, not after the choice.
                await self.editor.flush()
                guard var open = self.openNote, open.conflict != nil else { return }
                open.takeTheirs()
                let shown = await self.editor.reload(
                    noteID: self.openNoteID, revision: self.revisions[self.openNoteID, default: 0], text: open.text
                )
                if shown {
                    self.openNote = open
                    return
                }
            }
            logger.error("Take Theirs could not reach the editor")
        }
    }

    private func show(_ path: String) async {
        guard let vault else { return }
        switch tree.item(at: path) {
        case .note(let note):
            guard openNote?.note != note else {
                shown = .note(note)
                return
            }
            guard await leaveOpenNote() else {
                // The user stays with edits that could not be saved: back to that Note.
                if let open = openNote {
                    shown = .note(open.note)
                    selection = open.note.path
                }
                return
            }
            shown = .note(note)
            do {
                let open = try vault.open(note)
                openNote = open
                openNoteID = UUID().uuidString
                // Revisions matter only for the Note open and the one just left.
                revisions = revisions.filter { $0.key == previousNoteID }
                editor.load(noteID: openNoteID, text: open.text)
                failure = nil
            } catch {
                openNote = nil
                editor.clear()
                logger.error("Cannot read a Note: \(error.localizedDescription, privacy: .public)")
                failure = "Cannot read \(note.name): \(error.localizedDescription)"
            }
        case .attachment(let attachment):
            shown = .attachment(attachment)
            failure = nil
        case .folder, nil: break
        }
    }

    private func edited(noteID: String, revision: Int, text: String) {
        guard revision > revisions[noteID, default: 0] else {
            logger.info("Stale report from the editor, ignored")
            return
        }
        revisions[noteID] = revision
        if openNote != nil, noteID == openNoteID {
            openNote?.edit(text)
            pendingSave?.cancel()
            scheduleSave()
        } else if var previous = previousNote, noteID == previousNoteID {
            // Never into the Note now open, never into a file reopened from scratch.
            previous.edit(text)
            do {
                try previous.save()
            } catch {
                logger.error("Cannot save the last keys of a Note: \(error.localizedDescription, privacy: .public)")
                saveFailure = "The last keys typed in \(previous.note.name) could not be saved: \(error.localizedDescription)"
            }
            previousNote = previous
        } else {
            logger.error("Report for a Note no longer open, ignored")
        }
    }

    /// Before the editor gets another Note, the Vault changes or the app quits: saves, and asks
    /// the user when that fails. False when the user chose to stay with the edits.
    private func leaveOpenNote() async -> Bool {
        await editor.flush()
        pendingSave?.cancel()
        await saveNow()
        if var open = openNote, open.isDeleted {
            // Its text is only on screen: leaving would lose it unless the user says so.
            switch askAboutDeletedNote(open) {
            case .stay:
                return false
            case .discard:
                break
            case .replace:
                do {
                    try open.saveAgain()
                    openNote = open
                } catch {
                    saveFailure = "Cannot save \(open.note.name): \(error.localizedDescription)"
                    return false
                }
            }
            previousNote = nil
            saveFailure = nil
            return true
        }
        guard var open = openNote, open.hasUnsavedEdits else {
            previousNote = openNote
            previousNoteID = openNoteID
            return true
        }
        switch askAboutUnsavedEdits(in: open) {
        case .stay:
            return false
        case .discard:
            previousNote = nil
            saveFailure = nil
            return true
        case .replace:
            do {
                try open.keepMine()
                saveFailure = nil
                openNote = open
                previousNote = open
                previousNoteID = openNoteID
                return true
            } catch {
                saveFailure = "Cannot save \(open.note.name): \(error.localizedDescription)"
                return false
            }
        }
    }

    private enum UnsavedChoice { case stay, discard, replace }

    private func askAboutDeletedNote(_ open: OpenNote) -> UnsavedChoice {
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "\(open.note.name) was deleted by another app"
        alert.informativeText = "Its text is only in Noteki. Leaving it without saving it again loses it."
        alert.addButton(withTitle: "Keep Editing")
        alert.addButton(withTitle: "Discard")
        alert.addButton(withTitle: "Save Again")
        switch alert.runModal() {
        case .alertSecondButtonReturn: return .discard
        case .alertThirdButtonReturn: return .replace
        default: return .stay
        }
    }

    private func askAboutUnsavedEdits(in open: OpenNote) -> UnsavedChoice {
        let inConflict = open.conflict != nil
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "Your edits to \(open.note.name) are not saved"
        alert.informativeText = inConflict
            ? "Another app changed the same lines while you were writing. Replacing them with your version erases that change."
            : saveFailure ?? "The file could not be written."
        alert.addButton(withTitle: "Keep Editing")
        alert.addButton(withTitle: "Discard My Edits")
        if inConflict { alert.addButton(withTitle: "Replace With My Version") }
        switch alert.runModal() {
        case .alertSecondButtonReturn: return .discard
        case .alertThirdButtonReturn: return .replace
        default: return .stay
        }
    }

    private func scheduleSave() {
        pendingSave?.cancel()
        pendingSave = Task {
            try? await Task.sleep(for: Self.saveDelay)
            guard !Task.isCancelled else { return }
            await saveNow()
        }
    }

    /// Saves the open Note. A file another app changed meanwhile is merged first, then saved;
    /// during a Conflict nothing is written and the banner waits for the user.
    private func saveNow() async {
        guard var open = openNote, open.hasUnsavedEdits else { return }
        var changedOnDisk = false
        do {
            try open.save()
            saveFailure = nil
        } catch OpenNote.SaveError.changedOnDisk {
            changedOnDisk = true
        } catch OpenNote.SaveError.conflict, OpenNote.SaveError.deleted {
            // The banner is up: nothing to add.
        } catch OpenNote.SaveError.missing {
            // Renamed or deleted: the file event follows, and with it the save or the banner.
        } catch {
            logger.error("Cannot save a Note: \(error.localizedDescription, privacy: .public)")
            saveFailure = "Cannot save \(open.note.name): \(error.localizedDescription)"
        }
        openNote = open
        if changedOnDisk { await noticeChanges(at: [open.note.path]) }
    }

    private func afterTransition(_ work: @escaping @MainActor () async -> Void) {
        let previous = transition
        transition = Task {
            await previous?.value
            await work()
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
