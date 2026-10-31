import AppKit
import NotekiCore
import Observation
import Synchronization
import os

private let logger = Logger(subsystem: "app.noteki.noteki", category: "vault")

/// The open Vault and what it shows: a Note or an Attachment. The Vault's location is
/// remembered in the app's preferences, outside the Vault (ADR 0002). The Note shown is open
/// for writing and saved by itself about a second after the last change.
@MainActor
@Observable
final class AppModel {
    private static let vaultPathKey = "vaultPath"
    /// The Notes opened recently, per Vault: its path → theirs, the latest first.
    private static let recentNotesKey = "recentNotes"
    private static let recentNotesKept = 50

    private(set) var vault: Vault?
    private(set) var tree: [VaultItem] = []
    /// The Vault's files by name, to resolve Wikilinks; rebuilt whenever Files is listed.
    private var index: VaultIndex? {
        didSet {
            markUnresolvedLinks()
            updateBacklinks()
        }
    }
    /// The Wikilinks written in each Note, read once the right sidebar is first shown.
    private let links = ScannedNotes<LinkMap>(name: "Links")
    /// The Tags of each Note, read once the Tags pane is first shown.
    private let tagged = ScannedNotes<TagMap>(name: "Tags")
    /// Reads of links not finished: the sidebar says so rather than "No Backlinks".
    var isReadingLinks: Bool { links.isReading }
    /// The Tags are not known yet: being read, or Files not listed.
    var isReadingTags: Bool { tagged.isReading || !tagged.hasRead }
    /// The Tags as a tree, and the Notes of the Tag chosen, for the Tags pane.
    private(set) var tagTree: [TagNode] = []
    /// The editor's requests for the Vault's Tags made before they were read: answered once they are.
    private var tagAnswers: [@MainActor ([String]) -> Void] = []
    private(set) var notesOfSelectedTag: [Note] = []
    /// The path of the Tag chosen in the tree.
    var selectedTag: String? {
        didSet { updateTagNotes() }
    }
    enum LeftPane: String { case files, tags, search }
    /// What the left sidebar shows, remembered in the preferences. Tags are read the first time
    /// the Tags pane is shown.
    var leftPane = LeftPane(rawValue: UserDefaults.standard.string(forKey: AppModel.leftPaneKey) ?? "") ?? .files {
        didSet {
            UserDefaults.standard.set(leftPane.rawValue, forKey: Self.leftPaneKey)
            guard leftPane != oldValue, let index, let vault else { return }
            switch leftPane {
            case .tags: tagged.listed(index.notes, in: vault, isWanted: true)
            // The files may have changed while the pane was hidden: the results are found again.
            case .search: if !searchText.isEmpty { search() }
            case .files: break
            }
        }
    }
    private static let leftPaneKey = "leftPane"
    /// What the Search pane looks for; searched again a moment after the typing stops.
    var searchText = "" {
        didSet { if searchText != oldValue { search(after: .milliseconds(250)) } }
    }
    private(set) var searchResults = SearchResults.none
    private(set) var isSearching = false
    private var searchTask: Task<Void, Never>?
    /// ⌘⇧F asked for the Search field: the pane gives it the keyboard and sets it back.
    var wantsSearchField = false
    /// The Backlinks of the Note in front, for the right sidebar, and the Note they were found for.
    private(set) var backlinks: [Backlink] = []
    private(set) var backlinksNote: Note?
    private var backlinkQuery: Task<Void, Never>?
    /// The Backlinks shown are those of the Note in front: an empty list is an answer, not a wait.
    var backlinksAreFound: Bool { backlinksNote != nil && backlinksNote == noteInFront }
    /// The right sidebar, remembered across launches in the app's preferences. Hidden at first, as
    /// in Obsidian. The Notes' links are read when the Backlinks are first shown, not before: a
    /// Vault of 5,000 Notes would hold about 4 MB for a pane nobody opened (SPEC, Budget).
    var isShowingSidebar = UserDefaults.standard.bool(forKey: AppModel.sidebarKey) {
        didSet {
            UserDefaults.standard.set(isShowingSidebar, forKey: Self.sidebarKey)
            if isShowingSidebar != oldValue { rightSidebarChanged() }
        }
    }
    private static let sidebarKey = "rightSidebar"
    enum RightPane: String { case outline, backlinks }
    /// What the right sidebar shows, remembered in the preferences.
    var rightPane = RightPane(rawValue: UserDefaults.standard.string(forKey: AppModel.rightPaneKey) ?? "") ?? .outline {
        didSet {
            UserDefaults.standard.set(rightPane.rawValue, forKey: Self.rightPaneKey)
            if rightPane != oldValue { rightSidebarChanged() }
        }
    }
    private static let rightPaneKey = "rightPane"
    private var isShowingBacklinks: Bool { isShowingSidebar && rightPane == .backlinks }
    /// A Loose file's window has no right sidebar, whatever the preferences say.
    private var isShowingOutline: Bool { isShowingSidebar && rightPane == .outline && !isLooseFile }
    /// ⌘E: the user asked for Source mode in the Tab in front; each Tab keeps its own.
    var prefersSourceMode: Bool {
        get { tabs.front.map { sourceModeTabs.contains($0.id) } ?? false }
        set {
            guard let id = tabs.front?.id else { return }
            if newValue { sourceModeTabs.insert(id) } else { sourceModeTabs.remove(id) }
            updateSourceMode()
        }
    }
    /// The Tabs in Source mode, not remembered across launches.
    private var sourceModeTabs: Set<Tab.ID> = []
    /// A Note above this size opens in Source mode: Live preview would follow it slowly.
    private static let largestPreviewed = 5_000_000
    /// The Note open is too large for Live preview: Source mode, whatever was asked.
    private(set) var isSourceModeForced = false {
        didSet { updateSourceMode() }
    }
    var isSourceMode: Bool { prefersSourceMode || isSourceModeForced }

    private func updateSourceMode() {
        editor.isSourceMode = isSourceMode
    }

    /// The Headings of the Note in front, as it is typed, for the Outline.
    private(set) var outline: [Heading] = []

    private func rightSidebarChanged() {
        updateOutline()
        guard isShowingBacklinks, let index, let vault else { return updateBacklinks() }
        links.listed(index.notes, in: vault, isWanted: true)
        updateBacklinks()
    }

    /// From the text of the open Note as the editor last reported it: the Outline follows typing.
    private func updateOutline() {
        guard isShowingOutline, let open = openNote, noteInFront == open.note else {
            if !outline.isEmpty { outline = [] }
            return
        }
        let headings = open.outline
        if headings != outline { outline = headings }
    }

    /// Moves the editor to a Heading of the Note in front.
    func go(to heading: Heading) {
        guard let note = noteInFront else { return }
        open(note, atLine: heading.line)
    }
    /// A Backlink chosen: its Note opens with the cursor on that line once it is in front.
    private var lineToShow: (note: Note, line: Int)?
    /// The Notes opened in this Vault, the latest first, offered first to complete a Wikilink
    /// and in the Quick switcher.
    private var recentNotes: [Note] = []
    /// Over the window, finding a Note by its name; only once Files is listed.
    private(set) var isShowingSwitcher = false
    /// What had the keyboard before the Quick switcher: closing it without a choice gives it back.
    private weak var responderBeforeSwitcher: NSResponder?
    /// Until SwiftUI has removed it, a little after `isShowingSwitcher` turns false.
    private var isSwitcherOnScreen = false
    /// The Note chosen in the Quick switcher: the editor takes the keyboard once it is shown.
    private var noteToFocus: Note?
    private var embedQuery: Task<Void, Never>?
    /// The links of the open Note, as the editor last reported them.
    private var openNoteLinks: [String] = []
    /// The window's Tabs and the one in front (SPEC, Window): the app keeps them for the next launch.
    private(set) var tabs = Tabs() {
        didSet { if tabs != oldValue, !isReplacingTabs { onTabsChanged(tabs) } }
    }
    private var isReplacingTabs = false
    @ObservationIgnored var onTabsChanged: (Tabs) -> Void = { _ in }
    /// The user chose another Vault in this window.
    @ObservationIgnored var onVaultChanged: () -> Void = {}
    /// The Notes of the Tabs behind, open as they were left (saved), and what the editor keeps
    /// their state under (ADR 0003). The file's changes reach them there.
    private var behind: [Tab.ID: (open: OpenNote, noteID: String)] = [:]
    /// The editor states to let go once the next Note is in the editor: those of Notes no Tab shows.
    private var statesToDrop: [String] = []
    /// While Files' selection follows the Tab in front: nothing to open for it.
    private var isFollowingTab = false
    /// The Note or Attachment the Tab in front shows, nil without a Tab.
    private(set) var shown: Shown? {
        didSet {
            editor.isInFront = isShowingNote
            updateBacklinks()
            updateOutline()
        }
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
        didSet {
            conflict = openNote.flatMap { open in open.conflict.map { .init(mine: open.text, theirs: $0.theirs) } }
            updateOutline()
            // Opened, reloaded from disk or typed past the limit; no Note, no limit.
            let forced = (openNote?.text.utf8.count ?? 0) > Self.largestPreviewed
            if forced != isSourceModeForced { isSourceModeForced = forced }
        }
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

    /// The path of the row selected in the Files pane: a Note, an Attachment or a folder. It
    /// follows the Tab in front; a click opens the file there, a ⌘-click in a new Tab.
    var selection: String? {
        didSet {
            guard selection != oldValue, !isFollowingTab else { return }
            let newTab = NSApp.currentEvent?.modifierFlags.contains(.command) == true
            // A ⌘-click on the row selected deselects it in the List: it still opens a new Tab.
            guard let selection else {
                if newTab, let oldValue { open(oldValue, inNewTab: true) }
                return
            }
            open(selection, inNewTab: newTab)
        }
    }

    /// The Loose file this window shows, nil for the Vault's window: a Markdown or text file
    /// from outside the Vault, alone, its folder neither listed nor remembered.
    let looseFile: URL?
    /// A Note, Attachment or folder to select once Files lists it: opened from the Finder or
    /// Takku before Files was listed, or just written by Takku.
    private var pendingReveal: (path: String, inNewTab: Bool)?

    init() {
        looseFile = nil
        connectEditor()
        connectScans()
        followExcludedFolders()
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

    /// Another window of the Vault (⌘⇧N): its own model and web view (ADR 0010).
    init(vault root: URL) {
        looseFile = nil
        connectEditor()
        connectScans()
        followExcludedFolders()
        activate(Vault(root: root))
    }

    init(looseFile: URL) {
        self.looseFile = looseFile
        connectEditor()
        connectScans()
        activate(Vault(root: looseFile.deletingLastPathComponent()))
        tabs.show(looseFile.lastPathComponent)
        afterTransition { await self.present() }
    }

    private var isLooseFile: Bool { looseFile != nil }

    @ObservationIgnored private var settingsObserver: (any NSObjectProtocol)?

    /// Excluded folders changed in Settings for this Vault: Files, the index and Search leave
    /// them out from now on. The Tabs inside a folder newly left out close, their Notes saved:
    /// no change on disk would reach them any more (ADR 0005).
    private func followExcludedFolders() {
        settingsObserver = NotificationCenter.default.addObserver(
            forName: AppSettings.excludedFoldersChanged, object: nil, queue: .main
        ) { [weak self] notification in
            let root = notification.object as? String
            MainActor.assumeIsolated {
                guard let self, let vault = self.vault, root == vault.root.path(percentEncoded: false) else { return }
                let updated = Vault(root: vault.root, excludedFolders: AppSettings.shared.excludedFolders(of: vault.root))
                let leftOut = updated.excludedFolders.subtracting(vault.excludedFolders)
                self.vault = updated
                self.editor.images.vault = updated
                self.afterTransition {
                    // Tabs show paths as on disk; the folders are kept lowercased.
                    for tab in self.tabs.all {
                        let path = tab.path.lowercased()
                        if let folder = leftOut.first(where: { path.hasPrefix($0 + "/") }) {
                            await self.closeTabs(under: String(tab.path.prefix(folder.count)))
                        }
                    }
                    await self.listTree(of: updated, refresh: true)
                }
            }
        }
    }

    /// Where the Loose file is now, after any Followed rename.
    var looseFileNow: URL? {
        guard let looseFile else { return nil }
        guard let vault, case .note(let note)? = shown else { return looseFile }
        return vault.root.appending(path: note.path)
    }

    private func connectEditor() {
        editor.onChange = { [weak self] noteID, revision, text in
            self?.edited(noteID: noteID, revision: revision, text: text)
        }
        editor.onFollowLink = { [weak self] inner, newTab in self?.followLink(inner, newTab: newTab) }
        editor.linkCompletions = { [weak self] in
            // Before Files is listed, or in a Loose file, there is nothing to offer.
            guard let self, let index else { return .none }
            return index.linkCompletions(recent: recentNotes)
        }
        editor.tagCompletions = { [weak self] answer in
            // Before Files is listed, or in a Loose file, there are no Tags to offer.
            guard let self, let index, let vault else { return answer([]) }
            tagAnswers.append(answer)
            // The Tags are read the first time they are needed, as for the Tags pane.
            tagged.listed(index.notes, in: vault, isWanted: true)
            answerTagCompletions()
        }
        editor.takesImages = { [weak self] in self?.takesImages ?? false }
        editor.onImages = { [weak self] images, point in self?.embed(images, at: point) }
        editor.onCheckLinks = { [weak self] noteID, inners in
            guard let self, noteID == openNoteID else { return }
            openNoteLinks = inners
            markUnresolvedLinks()
        }
    }

    /// Tells the editor which of the open Note's links lead nowhere, and what its Embeds show: again
    /// whenever the index changes, so a link turns resolved once its Note exists.
    private func markUnresolvedLinks() {
        guard let open = openNote else { return }
        // Without an index (Files not listed yet, a Loose file) nothing is known to be missing.
        let unresolved = index.map { index in
            openNoteLinks.filter { index.resolve(Wikilink(inner: $0).target, from: open.note) == nil }
        } ?? []
        editor.markUnresolved(noteID: openNoteID, inners: unresolved)
        showEmbeds()
    }

    /// Tells the editor what the open Note's Embeds show: the images (their size read from their
    /// files, away from the main thread) and the PDFs. A newer answer drops an older one.
    private func showEmbeds() {
        embedQuery?.cancel()
        guard let open = openNote, let index, let vault else { return }
        let noteID = openNoteID
        let inners = openNoteLinks
        embedQuery = Task {
            let files = await Task.detached {
                var files: [String: EmbeddedFile] = [:]
                for inner in inners {
                    if let file = vault.embed(inner, from: open.note, in: index) { files[inner] = file }
                }
                return files
            }.value
            guard !Task.isCancelled else { return }
            editor.showEmbeds(noteID: noteID, files: files)
        }
    }

    /// Pasted and dropped images are written next to the Note in front, once Files is listed:
    /// not in a Loose file's folder, which is no Vault. Otherwise WebKit gets them.
    private var takesImages: Bool {
        guard !isLooseFile, index != nil, let open = openNote else { return false }
        return noteInFront == open.note && editor.canInsert(into: openNoteID)
    }

    /// A Pasted image, or image files pasted or dropped on the Note in front: written in the
    /// Vault next to it (SPEC, Pasted image) and their Embeds into it.
    private func embed(_ images: EditorWebView.IncomingImages, at point: EditorCommand.Point?) {
        guard takesImages, let vault, let index, let open = openNote else { return }
        let embeds: String?
        do {
            switch images {
            case .pasted(let png): embeds = try vault.pasteImage(png, into: open.note)
            case .files(let files): embeds = try vault.dropFiles(files, onto: open.note, in: index)
            }
        } catch {
            logger.error("Cannot write the pasted or dropped images: \(error.localizedDescription, privacy: .public)")
            NSSound.beep()
            return
        }
        // Files with no image among them: nothing written.
        guard let embeds else { return NSSound.beep() }
        editor.insert(noteID: openNoteID, text: embeds, at: point)
        // Listed at once, so the Embeds show their images without waiting for the watcher.
        refreshTree(of: vault)
    }

    /// Opens the Note or Attachment a clicked Wikilink leads to, resolved from the open Note as
    /// Obsidian resolves it.
    private func followLink(_ inner: String, newTab: Bool) {
        // Before Files is listed, or in a Loose file, there is nothing to resolve against: not
        // an Unresolved link.
        guard let index else { return }
        let target = Wikilink(inner: inner).target
        if let item = index.resolve(target, from: openNote?.note) {
            reveal(item.path, inNewTab: newTab)
            return
        }
        // An Unresolved link: its Note is created where Obsidian would put it, then opened.
        createNote(for: target, from: openNote?.note, inNewTab: newTab)
    }

    /// Creates the Note a Wikilink's `target` would lead to from `source`, and opens it.
    private func createNote(for target: String, from source: Note?, inNewTab: Bool = false) {
        guard let vault else { return }
        do {
            let created = try vault.createNote(for: target, from: source)
            reveal(created.path, inNewTab: inNewTab)
        } catch Vault.CreateError.exists(let note) {
            // Created a moment ago, by a click before Files listed it or by another app.
            reveal(note.path, inNewTab: inNewTab)
        } catch {
            // An Attachment's name, a name Obsidian refuses, or outside the Vault.
            logger.info("Note not created: \(String(describing: error), privacy: .public)")
            NSSound.beep()
        }
    }

    // MARK: File operations (SPEC, Files)

    /// The row of Files whose name is being typed: a Note just made with ⌘N, or Rename. Its
    /// folders open, so the row is on screen.
    var renaming: String? {
        didSet {
            var folder = renaming.map(folderOf) ?? ""
            while !folder.isEmpty {
                expandedFolders.insert(folder)
                folder = folderOf(folder)
            }
        }
    }
    /// The folders open in Files, by their paths.
    var expandedFolders: Set<String> = []

    /// What Noteki did to the Vault's files, for every window of it to follow.
    enum FileChange {
        case moved(from: String, to: String)
        case trashed(String)
    }
    /// Hands this window's change to the other windows of the Vault (VaultWindows, ADR 0010).
    @ObservationIgnored var onFileChange: (FileChange) -> Void = { _ in }

    /// ⌘N: `Untitled.md` in the folder of the Note or Attachment in front, the root without one,
    /// opened in the Tab in front with its name in Files ready to type over.
    func newNote() {
        newNote(in: placeInFront.map(\.folder) ?? "")
    }

    /// ⌘⇧D: today's Daily note in the Tab in front, created when missing, the editor taking the
    /// keyboard to write in it.
    func openTodaysNote() {
        guard !isLooseFile else { return }
        operate { vault in
            let note = try vault.dailyNote(AppSettings.shared.dailyNote)
            self.reveal(note.path)
            self.focusEditorWhenShown(note)
            return nil
        }
    }

    /// From Files' menu: a new Note in `folder`, named there.
    func newNote(in folder: String) {
        guard !isLooseFile else { return }
        operate { vault in
            let note = try vault.newNote(in: folder)
            self.leftPane = .files
            self.renaming = note.path
            // Opened once Files lists it, right after.
            self.pendingReveal = (note.path, false)
            return nil
        }
    }

    /// From Files' menu: the row's name to type, once it is selected and its file opened, so
    /// that the selection taking the keyboard does not end the typing.
    func startRename(_ path: String) {
        if selection != path { selection = path }
        afterTransition { self.renaming = path }
    }

    /// From Files' menu: a new folder in `folder`, named there.
    func newFolder(in folder: String) {
        operate { vault in
            let path = try vault.newFolder(in: folder)
            self.leftPane = .files
            self.renaming = path
            return nil
        }
    }

    /// The name typed in Files: the file or folder takes it; Wikilinks to it are not updated (v1).
    func rename(_ path: String, to name: String) {
        renaming = nil
        let current = tree.item(at: path)?.editableName
        // Unchanged, as Escape or a click elsewhere leaves a new Note's name.
        if name.trimmingCharacters(in: .whitespaces) != current {
            operate { vault in .moved(from: path, to: try vault.rename(path, to: name)) }
        }
        focusIfNoteInFront(path)
    }

    /// Escape while typing a name: it stays as it was.
    func cancelRename(_ path: String) {
        renaming = nil
        focusIfNoteInFront(path)
    }

    /// The Note in front just named, as after ⌘N: the editor takes the keyboard for its text.
    private func focusIfNoteInFront(_ path: String) {
        if case .note(let note)? = shown, note.path == path { editor.focus() }
    }

    /// Files dragged onto a folder of Files, or onto the root; those not in the Vault are ignored.
    func move(_ paths: [String], into folder: String) {
        for path in paths where tree.item(at: path) != nil && folderOf(path) != folder {
            operate { vault in .moved(from: path, to: try vault.move(path, into: folder)) }
        }
    }

    /// Where a row of Files is on disk.
    func fileURL(of path: String) -> URL? {
        vault.map { $0.root.appending(path: path) }
    }

    /// Copy Path, Copy Relative Path and Copy Wikilink: the text on the pasteboard.
    func copy(_ text: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    /// Move to Trash: never deleted for good. The Tabs that showed it close.
    func trash(_ path: String) {
        operate { vault in
            try vault.trash(path)
            return .trashed(path)
        }
    }

    /// Runs a file operation after the Note open is saved, so no save goes to the old name, then
    /// has every window follow it and Files list it. A refusal is told, as the Finder tells it.
    private func operate(_ operation: @escaping (Vault) throws -> FileChange?) {
        afterTransition {
            guard let vault = self.vault else { return }
            await self.editor.flush()
            self.pendingSave?.cancel()
            await self.saveNow()
            do {
                if let change = try operation(vault) {
                    await self.apply(change)
                    self.onFileChange(change)
                }
            } catch {
                Self.tell(error)
            }
            await self.listTree(of: vault, refresh: true)
        }
    }

    /// Another window's file operation: in turn with this window's switches, its Tabs, Notes
    /// and Files follow.
    func follow(_ change: FileChange) {
        afterTransition {
            await self.apply(change)
            if let vault = self.vault { await self.listTree(of: vault, refresh: true) }
        }
    }

    private func apply(_ change: FileChange) async {
        switch change {
        case .moved(let old, let new): followMove(from: old, to: new)
        case .trashed(let path): await closeTabs(under: path)
        }
    }

    private func followMove(from old: String, to new: String) {
        tabs.renamed(from: old, to: new)
        if var open = openNote, pathAfterMove(open.note.path, from: old, to: new) != nil {
            let before = open.note
            open.moved(from: old, to: new)
            openNote = open
            if shown == .note(before) { shown = .note(open.note) }
        }
        if case .attachment(let attachment)? = shown, let path = pathAfterMove(attachment.path, from: old, to: new) {
            shown = Attachment(path: path).map(Shown.attachment)
        }
        for (tab, entry) in behind where pathAfterMove(entry.open.note.path, from: old, to: new) != nil {
            var open = entry.open
            open.moved(from: old, to: new)
            behind[tab] = (open, entry.noteID)
        }
        previousNote?.moved(from: old, to: new)
        recentNotes = recentNotes.map { note in pathAfterMove(note.path, from: old, to: new).map(Note.init(path:)) ?? note }
        rememberRecentNotes()
        if let renaming, let path = pathAfterMove(renaming, from: old, to: new) { self.renaming = path }
        expandedFolders = Set(expandedFolders.map { pathAfterMove($0, from: old, to: new) ?? $0 })
        if let selection, let path = pathAfterMove(selection, from: old, to: new) {
            isFollowingTab = true
            self.selection = path
            isFollowingTab = false
        }
    }

    /// The Tabs that showed what went to the Trash close, their Notes saved: the file is in the
    /// Trash as it was saved. A Note whose text is only on screen (edits not saved, a Conflict)
    /// keeps its Tab, and the banner of a deleted Note follows from the file event.
    private func closeTabs(under path: String) async {
        let isInTrash = { (shown: String) in pathAfterMove(shown, from: path, to: path) != nil }
        await editor.flush()
        await saveNow()
        let frontStays = openNote.map { $0.hasUnsavedEdits || $0.conflict != nil } ?? false
        for tab in tabs.all where isInTrash(tab.path) && !(tab.id == tabs.front?.id && frontStays) {
            if tab.id == tabs.front?.id {
                pendingSave?.cancel()
                if openNote != nil { statesToDrop.append(openNoteID) }
                openNote = nil
            } else if let entry = behind.removeValue(forKey: tab.id) {
                statesToDrop.append(entry.noteID)
            }
            tabs.close(tab.id)
            sourceModeTabs.remove(tab.id)
        }
        if let previous = previousNote, isInTrash(previous.note.path) { previousNote = nil }
        recentNotes.removeAll { isInTrash($0.path) }
        rememberRecentNotes()
        await present()
    }

    private static func tell(_ error: any Error) {
        let alert = NSAlert()
        switch error {
        case Vault.FileError.exists(let path):
            alert.messageText = "The name “\((path as NSString).lastPathComponent)” is already taken."
            alert.informativeText = "Choose another name."
        case Vault.FileError.invalidName:
            alert.messageText = "This name cannot be used."
            alert.informativeText = "A name cannot be empty, start with a dot, be a folder Noteki does not list (node_modules), or contain / \\ : [ ] # ^ or |."
        case Vault.FileError.intoItself:
            return NSSound.beep()
        case Vault.CreateError.outside:
            alert.messageText = "Today's Note would be in a folder the Vault leaves out."
            alert.informativeText = "Choose another folder for the Daily note in Settings."
        case Vault.CreateError.invalidName:
            alert.messageText = "Today's Note cannot have this name."
            alert.informativeText = "Check the Daily note's date format in Settings: the name cannot be empty, leave its folder, or hold \\ : [ ] # ^ or |."
        default:
            alert.messageText = "The operation could not be done."
            alert.informativeText = error.localizedDescription
        }
        alert.runModal()
    }

    var canShowSwitcher: Bool { index != nil }

    func showSwitcher(in window: NSWindow?) {
        // Before the switcher's field takes the keyboard.
        responderBeforeSwitcher = window?.firstResponder
        isShowingSwitcher = true
        isSwitcherOnScreen = true
    }

    /// Closed without a choice: the keyboard goes back where it was.
    func closeSwitcher() {
        isShowingSwitcher = false
    }

    /// Once SwiftUI has removed the switcher's field, which takes the keyboard with it.
    func switcherDisappeared() {
        isSwitcherOnScreen = false
        if let responder = responderBeforeSwitcher as? NSView {
            responder.window?.makeFirstResponder(responder)
        }
        responderBeforeSwitcher = nil
        focusIfShown()
    }

    /// The Notes the Quick switcher offers for what the user typed, the best first.
    func switcherMatches(for typed: String) -> [Note] {
        index?.switcherMatches(for: typed, recent: recentNotes, from: placeInFront) ?? []
    }

    /// The Note the Quick switcher offers to create for what the user typed, if any.
    func switcherNewNote(for typed: String) -> String? {
        guard let index, let vault else { return nil }
        return index.switcherNewNote(for: typed, from: placeInFront, in: vault)
    }

    /// Enter opens the Note in the Tab in front, ⌘Enter in a new Tab.
    func openFromSwitcher(_ note: Note, inNewTab: Bool = false) {
        isShowingSwitcher = false
        responderBeforeSwitcher = nil
        reveal(note.path, inNewTab: inNewTab)
        focusEditorWhenShown(note)
    }

    /// A bare name goes in the folder of the Note or Attachment in front, a path from the
    /// Vault's root, as a Wikilink's Note is created.
    func createFromSwitcher(_ name: String) {
        isShowingSwitcher = false
        responderBeforeSwitcher = nil
        guard let vault else { return }
        createNote(for: name, from: placeInFront)
        if let path = try? vault.newNotePath(for: name, from: placeInFront) { focusEditorWhenShown(Note(path: path)) }
    }

    /// Where a link or a new Note starts from: the Note in front, or an Attachment, as a Note
    /// would be in its folder (only the folder counts).
    private var placeInFront: Note? {
        switch shown {
        case .note(let note): note
        case .attachment(let attachment): Note(path: attachment.path)
        case nil: nil
        }
    }

    /// Gives the editor the keyboard once `note` is in front, so the user types into it at once.
    private func focusEditorWhenShown(_ note: Note) {
        noteToFocus = note
        focusIfShown()
    }

    /// The Note must be in front and the switcher gone, whichever comes last.
    private func focusIfShown() {
        guard !isSwitcherOnScreen, let note = noteToFocus, openNote?.note == note, shown == .note(note) else { return }
        noteToFocus = nil
        editor.focus()
    }

    /// Opens a Note or Attachment of the Vault in the Tab in front, or in a new one; a folder is
    /// selected in Files. At once if listed, otherwise once Files lists it (Takku's note just
    /// written).
    func reveal(_ path: String, inNewTab: Bool = false) {
        guard let item = tree.item(at: path) else {
            pendingReveal = (path, inNewTab)
            if !tree.isEmpty, let vault { refreshTree(of: vault) }
            return
        }
        if case .folder = item { return selection = path }
        open(path, inNewTab: inNewTab)
    }

    /// A file opened from the Finder or Takku: in the Tab that shows it, or in a new Tab, so the
    /// Tabs open stay as they were.
    func revealFromOutside(_ path: String) {
        if let tab = tabs.all.first(where: { $0.path == path }) { return select(tab.id) }
        reveal(path, inNewTab: tabs.front != nil)
    }

    /// Shows `path` in the Tab in front, or in a new Tab after it.
    private func open(_ path: String, inNewTab: Bool) {
        afterTransition {
            guard let item = self.tree.item(at: path) else { return }
            if case .folder = item { return }
            if !inNewTab, self.tabs.front?.path == path { return await self.present() }
            guard await self.leaveFront(keeping: inNewTab) else { return self.followFrontInFiles() }
            self.tabs.show(path, inNewTab: inNewTab)
            await self.present()
        }
    }

    /// Brings the Tab `id` in front, as it was left.
    func select(_ id: Tab.ID) {
        changeTab { $0.select(id) }
    }

    /// ⌃Tab and ⌃⇧Tab.
    func selectNextTab() { changeTab { $0.selectNext() } }
    func selectPreviousTab() { changeTab { $0.selectPrevious() } }

    /// ⌘⌥← and ⌘⌥→: what the Tab in front showed before, or after.
    func goBack() { navigate { $0.back() } }
    func goForward() { navigate { $0.forward() } }

    var canGoBack: Bool { tabs.front?.canGoBack == true }
    var canGoForward: Bool { tabs.front?.canGoForward == true }

    /// Another Tab in front: the Tab left keeps its Note, behind.
    private func changeTab(_ change: @escaping (inout Tabs) -> Void) {
        afterTransition {
            var tabs = self.tabs
            change(&tabs)
            guard tabs.front?.id != self.tabs.front?.id else { return }
            guard await self.leaveFront(keeping: true) else { return }
            self.tabs = tabs
            await self.present()
        }
    }

    /// Another file in the Tab in front: the Note it showed is let go.
    private func navigate(_ change: @escaping (inout Tabs) -> Void) {
        afterTransition {
            var tabs = self.tabs
            change(&tabs)
            guard tabs != self.tabs else { return }
            guard await self.leaveFront(keeping: false) else { return }
            self.tabs = tabs
            await self.present()
        }
    }

    /// ⌘W, or the ×: a Tab closed, its Note saved first.
    func closeTab(_ id: Tab.ID) {
        afterTransition {
            // A Note behind that only lives on screen (deleted by another app, or not saved) comes
            // in front first: closing its Tab asks the user, as for the Note in front.
            if id != self.tabs.front?.id, let entry = self.behind[id], entry.open.isDeleted || entry.open.hasUnsavedEdits {
                guard await self.leaveFront(keeping: true) else { return }
                self.tabs.select(id)
                await self.present()
            }
            if id == self.tabs.front?.id {
                guard await self.leaveFront(keeping: false) else { return }
                self.tabs.close(id)
                await self.present()
            } else {
                if let entry = self.behind.removeValue(forKey: id) { self.editor.drop(noteIDs: [entry.noteID]) }
                self.tabs.close(id)
            }
            self.sourceModeTabs.remove(id)
        }
    }

    /// The Tabs a launch brings back: those whose file is still there. Their Notes are open from
    /// now on, so that one renamed by Takku before its Tab comes in front is followed.
    func restore(_ saved: Tabs) {
        guard let vault else { return }
        tabs = saved.keeping { FileManager.default.fileExists(atPath: vault.root.appending(path: $0).path(percentEncoded: false)) }
        for tab in tabs.all where tab.path.lowercased().hasSuffix(".md") {
            if let open = try? vault.open(Note(path: tab.path)) { behind[tab.id] = (open, UUID().uuidString) }
        }
    }

    /// Before the Tab in front shows something else: its Note saved, or the user stays (false).
    /// `keeping`: the Tab stays open with this Note, behind; otherwise the Note is let go.
    private func leaveFront(keeping: Bool) async -> Bool {
        guard let front = tabs.front, openNote != nil else { return true }
        guard await leaveOpenNote() else { return false }
        // Edits the user chose to discard are not kept: the Tab reads its file again.
        if keeping, let open = openNote, !open.hasUnsavedEdits {
            behind[front.id] = (open, openNoteID)
        } else {
            statesToDrop.append(openNoteID)
        }
        openNote = nil
        return true
    }

    /// Files selects what the Tab in front shows.
    private func followFrontInFiles() {
        isFollowingTab = true
        selection = tabs.front?.path
        isFollowingTab = false
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

    /// Saves what the user typed before the app quits or the window closes. A Note behind that
    /// only lives on screen comes in front to ask. False when the user chose to stay.
    func prepareToQuit() async -> Bool {
        // In turn with switches and with a window closing: one question at a time.
        await withCheckedContinuation { continuation in
            afterTransition {
                for (id, entry) in self.behind where entry.open.isDeleted || entry.open.hasUnsavedEdits {
                    guard await self.leaveFront(keeping: true) else { return continuation.resume(returning: false) }
                    self.tabs.select(id)
                    await self.present()
                }
                continuation.resume(returning: await self.leaveOpenNote())
            }
        }
    }

    private func switchVault(to folder: URL) async {
        guard await leaveFront(keeping: false) else { return }
        activate(Vault(root: folder))
        onVaultChanged()
    }

    private func activate(_ chosen: Vault) {
        // With the folders the user excludes from it; a Loose file's folder is no Vault.
        let vault = isLooseFile ? chosen : Vault(root: chosen.root, excludedFolders: AppSettings.shared.excludedFolders(of: chosen.root))
        self.vault = vault
        // A Loose file's folder may be ~ or ~/Downloads: its window embeds nothing, serves nothing.
        editor.images.vault = isLooseFile ? nil : vault
        pendingReveal = nil
        watcher = VaultWatcher(root: vault.root) { [weak self] change in self?.filesChanged(change) }
        tree = []
        shown = nil
        openNote = nil
        index = nil
        backlinkQuery?.cancel()
        links.reset()
        tagged.reset()
        tagTree = []
        // Asked about the Vault left: an answer from this one would be wrong.
        tagAnswers = []
        notesOfSelectedTag = []
        selectedTag = nil
        backlinks = []
        backlinksNote = nil
        searchTask?.cancel()
        searchResults = .none
        isSearching = false
        previousNote = nil
        revisions = [:]
        behind = [:]
        statesToDrop = []
        sourceModeTabs = []
        editor.clear()
        // Without telling: the Tabs kept for this Vault are not overwritten before they come back.
        if !isLooseFile {
            isReplacingTabs = true
            tabs = Tabs()
            isReplacingTabs = false
        }
        followFrontInFiles()
        failure = nil
        recentNotes = []
        guard !isLooseFile else { return }
        UserDefaults.standard.set(vault.root.path(percentEncoded: false), forKey: Self.vaultPathKey)
        let remembered = UserDefaults.standard.dictionary(forKey: Self.recentNotesKey)?[vault.root.path(percentEncoded: false)]
        recentNotes = (remembered as? [String] ?? []).map(Note.init(path:))
        Task { await listTree(of: vault) }
    }

    /// Lists Files. `refresh` after a change on disk: a failure then keeps the last list and
    /// leaves the Note on screen, it is only logged.
    private func listTree(of vault: Vault, refresh: Bool = false) async {
        let started = ContinuousClock.now
        // Off the main thread: a large Vault takes a moment to list.
        let result = await Task.detached { Result { try vault.tree() } }.value
        guard self.vault?.root == vault.root else { return }
        switch result {
        case .success(let tree):
            self.tree = tree
            index = VaultIndex(tree: tree)
            readAfterListing(in: vault)
            // Read by scripts/measure.
            logger.info("Files listed: \(tree.noteCount) Notes in \((ContinuousClock.now - started).milliseconds) ms")
            if let pendingReveal, tree.item(at: pendingReveal.path) != nil {
                self.pendingReveal = nil
                reveal(pendingReveal.path, inNewTab: pendingReveal.inNewTab)
            } else if shown == nil, tabs.front != nil, !refresh {
                // The Tabs brought back at launch: the one in front is shown once Files is listed.
                afterTransition { await self.present() }
            }
        case .failure(let error):
            logger.error("Cannot list the Vault: \(error.localizedDescription, privacy: .public)")
            guard !refresh else { return }
            failure = "Cannot read the Vault \(vault.root.lastPathComponent): \(error.localizedDescription)"
        }
    }

    /// Files was listed: the panes that read the Notes' text read the new ones.
    private func readAfterListing(in vault: Vault) {
        guard let index else { return }
        links.listed(index.notes, in: vault, isWanted: isShowingBacklinks)
        tagged.listed(index.notes, in: vault, isWanted: leftPane == .tags)
        // Another Vault, or new Notes: the results are found again.
        if leftPane == .search, !searchText.isEmpty { search() }
    }

    private func connectScans() {
        links.didRead = { [weak self] notes, gone in
            guard let self else { return }
            // The Note in front links to others, not the other way round: saving it changes no one's
            // Backlinks but the Notes it links to, which are not in front.
            if !gone.isEmpty || notes.contains(where: { $0 != noteInFront }) { updateBacklinks(after: .milliseconds(300)) }
        }
        tagged.didRead = { [weak self] _, _ in self?.updateTags() }
    }

    private func updateTags() {
        let tree = tagged.map.tree()
        // Not assigned when the same: the outline is not drawn again for every save.
        if tree != tagTree { tagTree = tree }
        answerTagCompletions()
        // A Tag no Note has any more is no longer chosen.
        if let selectedTag, !tagTree.contains(where: { $0.contains(path: selectedTag) }) { self.selectedTag = nil }
        updateTagNotes()
    }

    /// The Tags, once read, to the editor's requests.
    private func answerTagCompletions() {
        guard !tagAnswers.isEmpty, tagged.hasRead, !tagged.isReading else { return }
        let names = tagged.map.completions()
        let answers = tagAnswers
        tagAnswers = []
        for answer in answers { answer(names) }
    }

    private func updateTagNotes() {
        notesOfSelectedTag = selectedTag.map { tagged.map.notes(tagged: $0) } ?? []
    }

    private var noteInFront: Note? {
        if case .note(let note)? = shown { note } else { nil }
    }

    /// Asks for the Backlinks of the Note in front: read away from the main thread, as they read
    /// the Notes that link to it. Only while the sidebar shows them. A newer question drops the
    /// answer to an older one; `delay` lets a burst of saves count as one.
    private func updateBacklinks(after delay: Duration = .zero) {
        backlinkQuery?.cancel()
        guard isShowingBacklinks, let index, let vault, let note = noteInFront else {
            backlinks = []
            backlinksNote = nil
            return
        }
        // Another Note: the last one's Backlinks must not stay under it while these are found.
        if backlinksNote != note { backlinks = [] }
        let linkMap = links.map
        backlinkQuery = Task {
            if delay > .zero { try? await Task.sleep(for: delay) }
            guard !Task.isCancelled else { return }
            let found = await Task.detached { linkMap.backlinks(to: note, using: index, in: vault) }.value
            guard !Task.isCancelled else { return }
            backlinks = found
            backlinksNote = note
        }
    }

    /// Searches the Vault's files for `searchText`, away from the main thread, after `delay`. A newer
    /// search drops an older one.
    private func search(after delay: Duration = .zero) {
        searchTask?.cancel()
        let query = SearchQuery(searchText)
        guard !query.isEmpty, let index, let vault else {
            searchResults = .none
            isSearching = false
            return
        }
        isSearching = true
        searchTask = Task {
            if delay > .zero { try? await Task.sleep(for: delay) }
            guard !Task.isCancelled else { return }
            // A newer search stops the reading of this one, not only its answer.
            let stopped = Stopped()
            let found = await withTaskCancellationHandler {
                await Task.detached { vault.search(query, in: index.notes, isCancelled: stopped.callAsFunction) }.value
            } onCancel: {
                stopped.stop()
            }
            guard !Task.isCancelled, self.vault?.root == vault.root else { return }
            searchResults = found
            isSearching = false
        }
    }

    /// The Search pane, ⌘⇧F: shown, and its field given the keyboard.
    func showSearch() {
        leftPane = .search
        wantsSearchField = true
    }

    /// Opens the Note of a Backlink with the cursor on the line that holds the link.
    func open(_ backlink: Backlink) {
        open(backlink.note, atLine: backlink.line)
    }

    /// Opens `note` with the cursor on `line` (from 1), or at its start, and gives it the keyboard.
    func open(_ note: Note, atLine line: Int?) {
        let newTab = NSApp.currentEvent?.modifierFlags.contains(.command) == true
        if shown == .note(note), !newTab {
            // Already in front: only the cursor moves.
            if let line { editor.goToLine(noteID: openNoteID, line: line) }
            return editor.focus()
        }
        lineToShow = line.map { (note, $0) }
        reveal(note.path, inNewTab: newTab)
        focusEditorWhenShown(note)
    }

    /// From the Vault's watcher: files changed on disk, by Noteki or another app.
    private func filesChanged(_ change: VaultWatcher.Change) {
        guard let vault else { return }
        var urls = change.urls
        if let looseFile {
            // A Loose file's folder may be ~ or ~/Downloads, busy and deep: only events in that
            // folder itself can concern the file, by name, before any path is resolved.
            let folderName = looseFile.deletingLastPathComponent().lastPathComponent
            urls = urls.filter { $0.deletingLastPathComponent().lastPathComponent == folderName }
        }
        var paths = vault.changedPaths(urls)
        // Events were lost or merged into folders: anything may have changed.
        if change.mustRescan, let open = openNote { paths.insert(open.note.path) }
        guard !paths.isEmpty else { return }
        // A Note changed on disk, saved by Noteki or not: its links are read again. Those new to
        // Files are read once Files lists them.
        let changedNotes = paths.compactMap { tree.item(at: $0)?.note }
        // An image replaced on disk: its new version and size, so the editor loads it again.
        if paths.contains(where: { Attachment(path: $0)?.kind == .image }) { showEmbeds() }
        links.changed(changedNotes, in: vault)
        tagged.changed(changedNotes, in: vault)
        // Results that may have changed with the files: a burst of saves counts as one search.
        if leftPane == .search, !searchText.isEmpty { search(after: .milliseconds(500)) }
        if !isLooseFile, change.mustRescan || paths.contains(where: { appearedOrDisappeared($0, in: vault) }) {
            // A Note or Attachment created or deleted by another app, such as Takku's new
            // Meeting notes: Files lists it without a relaunch.
            refreshTree(of: vault)
        }
        // In turn with a Tab switch: a Note moving behind meanwhile is found there.
        afterTransition {
            self.noticeChangesBehind(at: paths)
            // Case-insensitively: a rename that only changes letter case is reported under the new name.
            if let open = self.openNote,
               open.isDeleted || paths.contains(where: { $0.caseInsensitiveCompare(open.note.path) == .orderedSame }) {
                await self.noticeChanges(at: paths)
            }
        }
    }

    /// The Notes of the Tabs behind changed on disk: they follow a rename and take the file's
    /// text, as they hold no edits (saved when their Tab was left). The editor gets it when the
    /// Tab comes back.
    private func noticeChangesBehind(at paths: Set<String>) {
        for (tab, entry) in behind {
            let path = entry.open.note.path
            guard entry.open.isDeleted || paths.contains(where: { $0.caseInsensitiveCompare(path) == .orderedSame }) else { continue }
            var open = entry.open
            guard let change = try? open.noticeChanges(at: paths) else { continue }
            if open.note != entry.open.note {
                tabs.renamed(from: path, to: open.note.path)
                followRename(of: entry.open.note, to: open.note)
            }
            if case .reload(let reload) = change, open.apply(reload), open.hasUnsavedEdits {
                // Merged with keys that could not be saved: saved now.
                try? open.save()
            }
            behind[tab] = (open, entry.noteID)
        }
    }

    private func refreshTree(of vault: Vault) {
        pendingTreeRefresh?.cancel()
        pendingTreeRefresh = Task {
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled else { return }
            await listTree(of: vault, refresh: true)
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
            if let before, open.note != before {
                // A Followed rename of the Note in front: its Tab, and any other showing it, follow;
                // the window shows its new name and Files selects it.
                tabs.renamed(from: before.path, to: open.note.path)
                if shown == .note(before) { shown = .note(open.note) }
                followFrontInFiles()
            }
            if let before, open.note != before { followRename(of: before, to: open.note) }
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

    /// Shows what the Tab in front shows: its Note as it was left, read again when it was let
    /// go, or its Attachment; nothing without a Tab.
    private func present() async {
        defer {
            followFrontInFiles()
            if !statesToDrop.isEmpty {
                editor.drop(noteIDs: statesToDrop)
                statesToDrop = []
            }
        }
        guard let vault, let front = tabs.front else {
            shown = nil
            openNote = nil
            failure = nil
            behind = [:]
            statesToDrop = []
            editor.clear()
            return
        }
        // Taken now, so that a Note not opened never keeps it for later.
        let line = lineToShow.flatMap { $0.note.path == front.path ? $0.line : nil }
        lineToShow = nil
        let path = front.path
        // A Loose file is not listed: its Tab shows its one Note.
        let item = isLooseFile ? .note(Note(path: path)) : tree.item(at: path)
        switch item {
        case .note(let note):
            if let open = openNote, open.note == note {
                // Already in front: only the cursor moves.
                shown = .note(note)
                if let line { editor.goToLine(noteID: openNoteID, line: line) }
                focusIfShown()
                return
            }
            let kept = behind.removeValue(forKey: front.id)
            shown = .note(note)
            if let kept, kept.open.note == note {
                openNote = kept.open
                openNoteID = kept.noteID
                openNoteLinks = []
                editor.restore(noteID: kept.noteID, text: kept.open.text)
                if let line { editor.goToLine(noteID: kept.noteID, line: line) }
            } else {
                if let kept { statesToDrop.append(kept.noteID) }
                do {
                    let open = try vault.open(note)
                    openNote = open
                    openNoteID = UUID().uuidString
                    openNoteLinks = []
                    editor.load(noteID: openNoteID, text: open.text, line: line)
                } catch {
                    openNote = nil
                    logger.error("Cannot read a Note: \(error.localizedDescription, privacy: .public)")
                    failure = "Cannot read \(note.name): \(error.localizedDescription)"
                    return
                }
            }
            // Revisions matter only for the Notes the editor may still report about.
            let live = Set([openNoteID, previousNoteID] + behind.values.map(\.noteID))
            revisions = revisions.filter { live.contains($0.key) }
            failure = nil
            updateSourceMode()
            opened(note)
            focusIfShown()
        case .attachment(let attachment):
            shown = .attachment(attachment)
            openNote = nil
            failure = nil
        case .folder, nil:
            shown = nil
            openNote = nil
            failure = "\((path as NSString).lastPathComponent) is no longer in the Vault."
        }
    }

    /// Remembers `note` as the latest opened, outside the Vault (ADR 0002).
    private func opened(_ note: Note) {
        // Other windows of the Vault open Notes too: theirs count.
        if !isLooseFile, let vault, let remembered = UserDefaults.standard.dictionary(forKey: Self.recentNotesKey)?[vault.root.path(percentEncoded: false)] as? [String] {
            recentNotes = remembered.map(Note.init(path:))
        }
        recentNotes = [note] + recentNotes.filter { $0 != note }.prefix(Self.recentNotesKept - 1)
        rememberRecentNotes()
    }

    /// Still among the Notes opened lately, under its new name.
    private func followRename(of before: Note, to after: Note) {
        recentNotes = recentNotes.map { $0 == before ? after : $0 }
        rememberRecentNotes()
    }

    private func rememberRecentNotes() {
        guard !isLooseFile, let vault else { return }
        var all = UserDefaults.standard.dictionary(forKey: Self.recentNotesKey) ?? [:]
        all[vault.root.path(percentEncoded: false)] = recentNotes.map(\.path)
        UserDefaults.standard.set(all, forKey: Self.recentNotesKey)
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
        } else if let (tab, entry) = behind.first(where: { $0.value.noteID == noteID }) {
            // Keys typed in the instant before the Tab went behind.
            behind[tab] = (saveLastKeys(text, in: entry.open), noteID)
        } else if let previous = previousNote, noteID == previousNoteID {
            // Never into the Note now open, never into a file reopened from scratch.
            previousNote = saveLastKeys(text, in: previous)
        } else {
            logger.error("Report for a Note no longer open, ignored")
        }
    }

    /// Keys the editor reported for a Note no longer in front, saved at once.
    private func saveLastKeys(_ text: String, in note: OpenNote) -> OpenNote {
        var open = note
        open.edit(text)
        do {
            try open.save()
        } catch {
            logger.error("Cannot save the last keys of a Note: \(error.localizedDescription, privacy: .public)")
            saveFailure = "The last keys typed in \(open.note.name) could not be saved: \(error.localizedDescription)"
        }
        return open
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

extension Duration {
    var milliseconds: Int64 { components.seconds * 1000 + components.attoseconds / 1_000_000_000_000_000 }
}

extension [VaultItem] {
    var noteCount: Int {
        reduce(0) { count, item in
            switch item {
            case .folder(_, let children): count + children.noteCount
            case .note: count + 1
            case .attachment: count
            }
        }
    }

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

private extension TagNode {
    /// Whether this Tag or one under it has the path.
    func contains(path wanted: String) -> Bool {
        path == wanted || children.contains { $0.contains(path: wanted) }
    }
}

/// Set once, read from any thread: a search's reading stops when it is.
private final class Stopped: Sendable {
    private let flag = Mutex(false)

    func stop() { flag.withLock { $0 = true } }
    func callAsFunction() -> Bool { flag.withLock { $0 } }
}
