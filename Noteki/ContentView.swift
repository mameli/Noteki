import NotekiCore
import SwiftUI

struct ContentView: View {
    @Bindable var model: AppModel
    /// Hands the app a way to bring this window back, once it exists.
    let registerShowMainWindow: (@escaping () -> Void) -> Void
    /// False while a Loose file opened at launch is on screen: no Vault is not a question then.
    let shouldAskForVault: () -> Bool
    let windows: VaultWindows
    /// For a window after the first: its id among the Vault's windows.
    var windowID: UUID?
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismiss) private var dismiss

    /// Shown again for ⌘⇧F: Search is in the sidebar.
    @State private var columns = NavigationSplitViewVisibility.automatic

    var body: some View {
        NavigationSplitView(columnVisibility: $columns) {
            LeftSidebar(model: model)
                .navigationSplitViewColumnWidth(min: 200, ideal: 260)
        } detail: {
            if model.vault == nil {
                ContentUnavailableView {
                    Label("No Vault", systemImage: "folder")
                } description: {
                    Text(AppModel.chooseVaultMessage)
                } actions: {
                    Button("Choose Vault…") { model.chooseVault() }
                }
            } else {
                VStack(spacing: 0) {
                    if !model.tabs.all.isEmpty { TabBar(model: model) }
                    NoteDetail(model: model)
                }
                    .inspector(isPresented: $model.isShowingSidebar) {
                        RightSidebar(model: model)
                            .inspectorColumnWidth(min: 220, ideal: 280, max: 420)
                    }
            }
        }
        .toolbar {
            if model.vault != nil {
                ToolbarItem {
                    Button("Right Sidebar", systemImage: "sidebar.trailing") { model.isShowingSidebar.toggle() }
                        .help("Show or hide the Outline and the Backlinks")
                }
            }
        }
        .overlay(alignment: .top) {
            if model.isShowingSwitcher {
                QuickSwitcher(model: model)
            }
        }
        .onChange(of: model.wantsSearchField) { _, wants in
            if wants, columns == .detailOnly { columns = .all }
        }
        // A name to type in Files, as after ⌘N: the sidebar shows.
        .onChange(of: model.renaming) { _, renaming in
            if renaming != nil, columns == .detailOnly { columns = .all }
        }
        .navigationTitle(model.shown?.name ?? model.vault?.root.lastPathComponent ?? "Noteki")
        .focusedSceneValue(\.noteEditor, model.isShowingNote ? model.editor : nil)
        .focusedSceneValue(\.noteWindow, model.isShowingNote ? model : nil)
        .focusedSceneValue(\.vaultWindow, model)
        // Another Vault: the other windows showed the one left; the first opens those kept for it.
        .onChange(of: windows.vaultChanges) {
            if windowID != nil { dismiss() } else { openRestoredWindows() }
        }
        .onDisappear { if let windowID { windows.closed(windowID) } }
        .onAppear {
            guard windowID == nil else { return }
            // Files opened from the Finder bring this window back even after it was closed.
            registerShowMainWindow { openWindow(id: "main") }
            windows.reopen = { openWindow(id: "vault", value: $0) }
            // The other windows of the last launch, with their Tabs.
            openRestoredWindows()
            guard model.vault == nil else { return }
            Task {
                // A file opened at launch arrives just after this window appears.
                try? await Task.sleep(for: .milliseconds(400))
                if model.vault == nil, shouldAskForVault() { model.chooseVault() }
            }
        }
    }

    private func openRestoredWindows() {
        for tabs in windows.takeToRestore() {
            if let id = windows.add(showing: tabs) { openWindow(id: "vault", value: id) }
        }
    }
}

/// A Loose file in its own window: the editor without Files.
struct LooseFileView: View {
    @Bindable var model: AppModel
    let file: URL

    var body: some View {
        NoteDetail(model: model)
            .navigationTitle(model.shown?.name ?? file.deletingPathExtension().lastPathComponent)
            .navigationDocument(model.looseFileNow ?? file)
            .focusedSceneValue(\.noteEditor, model.isShowingNote ? model.editor : nil)
            .focusedSceneValue(\.noteWindow, model.isShowingNote ? model : nil)
            .frame(minWidth: 480, minHeight: 360)
    }
}

/// The editor and what stands in front of it: placeholders, an Attachment, the banners.
struct NoteDetail: View {
    @Bindable var model: AppModel
    @State private var isShowingDifferences = false

    var body: some View {
        // The editor stays alive under the placeholders: its web view lives as long as the
        // window (ADR 0003) instead of loading again after every message.
        EditorView(editor: model.editor)
            .overlay {
                if let failure = model.failure {
                    placeholder(ContentUnavailableView(
                        "Cannot Open", systemImage: "exclamationmark.triangle", description: Text(failure)
                    ))
                } else if case .attachment(let attachment)? = model.shown, let vault = model.vault {
                    AttachmentView(attachment: attachment, url: vault.url(of: attachment))
                        .background(.background)
                } else if model.shown == nil {
                    placeholder(ContentUnavailableView("No Note Selected", systemImage: "doc.text"))
                }
            }
            .overlay(alignment: .bottom) {
                if model.isOpenNoteDeleted, case .note(let note)? = model.shown {
                    DeletedBanner(noteName: note.name, saveAgain: model.saveAgain)
                } else if model.conflict != nil, case .note(let note)? = model.shown {
                    ConflictBanner(
                        noteName: note.name,
                        keepMine: model.keepMine,
                        takeTheirs: model.takeTheirs,
                        showDifferences: { isShowingDifferences = true }
                    )
                } else if let saveFailure = model.saveFailure {
                    Label(saveFailure, systemImage: "exclamationmark.triangle.fill")
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(.red.opacity(0.9), in: .rect(cornerRadius: 8))
                        .foregroundStyle(.white)
                        .padding()
                }
            }
            .sheet(isPresented: $isShowingDifferences) {
                if let conflict = model.conflict {
                    DifferencesView(conflict: conflict) { isShowingDifferences = false }
                }
            }
            .onChange(of: model.conflict == nil) { _, resolved in
                if resolved { isShowingDifferences = false }
            }
    }

    private func placeholder(_ content: some View) -> some View {
        content
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(.background)
    }
}

/// The Vault's folders and files. A click opens, ⌘-click in a new Tab; the menu creates,
/// renames and moves to the Trash; dragging a row onto a folder moves it there, onto the empty
/// space below to the root.
struct FilesPane: View {
    @Bindable var model: AppModel
    @FocusState private var hasKeyboard: Bool

    var body: some View {
        List(selection: $model.selection) {
            FileRows(items: model.tree, model: model)
        }
        .focused($hasKeyboard)
        .contextMenu(forSelectionType: String.self) { paths in
            let item = paths.first.flatMap { model.tree.item(at: $0) }
            // In the folder clicked, the folder of the file clicked, or the root.
            let folder = item.map { $0.children != nil ? $0.path : folderOf($0.path) } ?? ""
            Button("New Note") { model.newNote(in: folder) }
            Button("New Folder") { model.newFolder(in: folder) }
            if let item {
                Divider()
                if let url = model.fileURL(of: item.path) {
                    Button("Reveal in Finder") { NSWorkspace.shared.activateFileViewerSelecting([url]) }
                    Divider()
                    Button("Copy Path") { model.copy(url.path(percentEncoded: false)) }
                }
                Button("Copy Relative Path") { model.copy(item.path) }
                if item.children == nil {
                    // As Obsidian writes it: a Note without its extension, an Attachment with.
                    Button("Copy Wikilink") { model.copy("[[\(item.isNote ? item.editableName : item.name)]]") }
                }
                Divider()
                Button("Rename") { model.startRename(item.path) }
                Button("Move to Trash") { model.trash(item.path) }
            }
        }
        .background {
            // ⌘⌫, as in the Finder, only while Files has the keyboard: in the editor it deletes
            // to the start of the line. Delete alone is too easy to press.
            Button("Move to Trash") { if let selection = model.selection { model.trash(selection) } }
                .keyboardShortcut(.delete, modifiers: .command)
                .disabled(!hasKeyboard || model.selection == nil || model.renaming != nil)
                .opacity(0)
                .accessibilityHidden(true)
        }
        .dropDestination(for: String.self) { paths, _ in
            model.move(paths, into: "")
            return true
        }
    }
}

/// The rows of a level of Files, folders opened as the model keeps them: a row to name, after
/// ⌘N or New Folder, has its folders opened.
private struct FileRows: View {
    let items: [VaultItem]
    @Bindable var model: AppModel

    var body: some View {
        ForEach(items) { item in
            if let children = item.children {
                DisclosureGroup(isExpanded: isExpanded(item.path)) {
                    // A level inside a level: its type cannot be named within itself.
                    AnyView(FileRows(items: children, model: model))
                } label: {
                    FileRow(item: item, model: model)
                        // The whole width of the row, not only its name.
                        .frame(maxWidth: .infinity, alignment: .leading)
                        // A click anywhere on a folder's row opens or closes it, not only on its
                        // triangle; the row is selected as well.
                        .contentShape(Rectangle())
                        .simultaneousGesture(TapGesture().onEnded { isExpanded(item.path).wrappedValue.toggle() })
                }
            } else {
                FileRow(item: item, model: model)
            }
        }
    }

    private func isExpanded(_ folder: String) -> Binding<Bool> {
        Binding(
            get: { model.expandedFolders.contains(folder) },
            set: { open in
                if open { model.expandedFolders.insert(folder) } else { model.expandedFolders.remove(folder) }
            }
        )
    }
}

/// A row of Files: its name, or a field to type a new one.
private struct FileRow: View {
    let item: VaultItem
    @Bindable var model: AppModel
    @State private var name = ""
    @FocusState private var isTyping: Bool

    var body: some View {
        Group {
            if model.renaming == item.path {
                Label {
                    TextField("Name", text: $name)
                        .focused($isTyping)
                        .onSubmit(commit)
                        .onExitCommand { model.cancelRename(item.path) }
                        .onAppear {
                            name = item.editableName
                            isTyping = true
                        }
                        // A click elsewhere keeps the name typed, as in the Finder.
                        .onChange(of: isTyping) { _, typing in if !typing { commit() } }
                } icon: {
                    Image(systemName: item.symbolName)
                }
            } else {
                Label(item.name, systemImage: item.symbolName)
            }
        }
        .draggable(item.path)
        // Onto a file: into its folder.
        .dropDestination(for: String.self) { paths, _ in
            model.move(paths, into: item.children != nil ? item.path : folderOf(item.path))
            return true
        }
    }

    private func commit() {
        guard model.renaming == item.path else { return }
        model.rename(item.path, to: name)
    }
}

extension VaultItem: @retroactive Identifiable {
    public var id: String { path }

    /// For the Files pane's outline: nil for files, so they show no disclosure triangle.
    var children: [VaultItem]? {
        switch self {
        case .folder(_, let children): children
        case .note, .attachment: nil
        }
    }

    /// The name as Files lets the user type it: a Note's and an Attachment's without extension.
    var editableName: String {
        switch self {
        case .folder: name
        case .note(let note): note.name
        case .attachment(let attachment): (attachment.name as NSString).deletingPathExtension
        }
    }

    var isNote: Bool {
        if case .note = self { true } else { false }
    }

    var symbolName: String {
        switch self {
        case .folder: "folder"
        case .note: "doc.text"
        case .attachment(let attachment):
            switch attachment.kind {
            case .image: "photo"
            case .pdf: "doc.richtext"
            }
        }
    }
}
