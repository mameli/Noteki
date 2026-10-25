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
    /// ⌘E: the user asked for Source mode in this window (Tabs keep their own, ticket 25).
    var prefersSourceMode = false {
        didSet { updateSourceMode() }
    }
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
    /// The Note or Attachment selected, nil before the first choice.
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

    /// The path of the row selected in the Files pane: a Note, an Attachment or a folder.
    var selection: String? {
        didSet {
            guard selection != oldValue, let selection else { return }
            afterTransition { await self.show(selection) }
        }
    }

    /// The Loose file this window shows, nil for the Vault's window: a Markdown or text file
    /// from outside the Vault, alone, its folder neither listed nor remembered.
    let looseFile: URL?
    /// A Note, Attachment or folder to select once Files lists it: opened from the Finder or
    /// Takku before Files was listed, or just written by Takku.
    private var pendingReveal: String?

    init() {
        looseFile = nil
        connectEditor()
        connectScans()
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

    init(looseFile: URL) {
        self.looseFile = looseFile
        connectEditor()
        connectScans()
        activate(Vault(root: looseFile.deletingLastPathComponent()))
        let note = Note(path: looseFile.lastPathComponent)
        afterTransition { await self.show(note) }
    }

    private var isLooseFile: Bool { looseFile != nil }

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
        editor.onFollowLink = { [weak self] inner in self?.followLink(inner) }
        editor.linkCompletions = { [weak self] in
            // Before Files is listed, or in a Loose file, there is nothing to offer.
            guard let self, let index else { return .none }
            return index.linkCompletions(recent: recentNotes)
        }
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

    /// Opens the Note or Attachment a clicked Wikilink leads to, resolved from the open Note as
    /// Obsidian resolves it.
    private func followLink(_ inner: String) {
        // Before Files is listed, or in a Loose file, there is nothing to resolve against: not
        // an Unresolved link.
        guard let index else { return }
        let target = Wikilink(inner: inner).target
        if let item = index.resolve(target, from: openNote?.note) {
            reveal(item.path)
            return
        }
        // An Unresolved link: its Note is created where Obsidian would put it, then opened.
        createNote(for: target, from: openNote?.note)
    }

    /// Creates the Note a Wikilink's `target` would lead to from `source`, and opens it.
    private func createNote(for target: String, from source: Note?) {
        guard let vault else { return }
        do {
            let created = try vault.createNote(for: target, from: source)
            reveal(created.path)
        } catch Vault.CreateError.exists(let note) {
            // Created a moment ago, by a click before Files listed it or by another app.
            reveal(note.path)
        } catch {
            // An Attachment's name, a name Obsidian refuses, or outside the Vault.
            logger.info("Note not created: \(String(describing: error), privacy: .public)")
            NSSound.beep()
        }
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

    func openFromSwitcher(_ note: Note) {
        isShowingSwitcher = false
        responderBeforeSwitcher = nil
        reveal(note.path)
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

    /// A Note, Attachment or folder of the Vault opened from the Finder or Takku: selected in
    /// Files, at once if listed, otherwise once Files lists it (Takku's note just written).
    func reveal(_ path: String) {
        guard tree.item(at: path) == nil else {
            selection = path
            return
        }
        pendingReveal = path
        if !tree.isEmpty, let vault { refreshTree(of: vault) }
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
        // In turn with switches and with a window closing: one question at a time.
        await withCheckedContinuation { continuation in
            afterTransition { continuation.resume(returning: await self.leaveOpenNote()) }
        }
    }

    private func switchVault(to folder: URL) async {
        guard await leaveOpenNote() else { return }
        activate(Vault(root: folder))
    }

    private func activate(_ vault: Vault) {
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
        notesOfSelectedTag = []
        selectedTag = nil
        backlinks = []
        backlinksNote = nil
        searchTask?.cancel()
        searchResults = .none
        isSearching = false
        previousNote = nil
        revisions = [:]
        editor.clear()
        selection = nil
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
            if let pendingReveal, tree.item(at: pendingReveal) != nil {
                self.pendingReveal = nil
                selection = pendingReveal
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
        // A Tag no Note has any more is no longer chosen.
        if let selectedTag, !tagTree.contains(where: { $0.contains(path: selectedTag) }) { self.selectedTag = nil }
        updateTagNotes()
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
        if shown == .note(note) {
            // Already in front: only the cursor moves.
            if let line { editor.goToLine(noteID: openNoteID, line: line) }
            return editor.focus()
        }
        lineToShow = line.map { (note, $0) }
        reveal(note.path)
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
        // Case-insensitively: a rename that only changes letter case is reported under the new name.
        if let open = openNote,
           open.isDeleted || paths.contains(where: { $0.caseInsensitiveCompare(open.note.path) == .orderedSame }) {
            afterTransition { await self.noticeChanges(at: paths) }
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
            if let before, open.note != before, shown == .note(before) {
                // A Followed rename of the Note in front: the window shows its new name and Files
                // selects it. Behind an Attachment, or once the user chose another row, nothing moves.
                shown = .note(open.note)
                if selection == before.path { selection = open.note.path }
            }
            if let before, open.note != before {
                // Still the Note opened last, under its new name.
                recentNotes = recentNotes.map { $0 == before ? open.note : $0 }
                rememberRecentNotes()
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
        switch tree.item(at: path) {
        case .note(let note):
            await show(note)
        case .attachment(let attachment):
            shown = .attachment(attachment)
            failure = nil
        case .folder, nil: break
        }
    }

    private func show(_ note: Note) async {
        guard let vault else { return }
        // Taken now, so that a Note not opened never keeps it for later.
        let line = lineToShow?.note == note ? lineToShow?.line : nil
        lineToShow = nil
        guard openNote?.note != note else {
            shown = .note(note)
            focusIfShown()
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
            openNoteLinks = []
            // Revisions matter only for the Note open and the one just left.
            revisions = revisions.filter { $0.key == previousNoteID }
            editor.load(noteID: openNoteID, text: open.text, line: line)
            failure = nil
            opened(note)
            focusIfShown()
        } catch {
            openNote = nil
            editor.clear()
            logger.error("Cannot read a Note: \(error.localizedDescription, privacy: .public)")
            failure = "Cannot read \(note.name): \(error.localizedDescription)"
        }
    }

    /// Remembers `note` as the latest opened, outside the Vault (ADR 0002).
    private func opened(_ note: Note) {
        recentNotes = [note] + recentNotes.filter { $0 != note }.prefix(Self.recentNotesKept - 1)
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

extension Duration {
    var milliseconds: Int64 { components.seconds * 1000 + components.attoseconds / 1_000_000_000_000_000 }
}

private extension [VaultItem] {
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
