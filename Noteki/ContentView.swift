import NotekiCore
import SwiftUI

struct ContentView: View {
    @Bindable var model: AppModel
    /// Hands the app a way to bring this window back, once it exists.
    let registerShowMainWindow: (@escaping () -> Void) -> Void
    /// False while a Loose file opened at launch is on screen: no Vault is not a question then.
    let shouldAskForVault: () -> Bool
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        NavigationSplitView {
            FilesPane(model: model)
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
                NoteDetail(model: model)
                    .inspector(isPresented: $model.isShowingSidebar) {
                        BacklinksPane(model: model)
                            .inspectorColumnWidth(min: 220, ideal: 280, max: 420)
                    }
            }
        }
        .toolbar {
            if model.vault != nil {
                ToolbarItem {
                    Button("Right Sidebar", systemImage: "sidebar.trailing") { model.isShowingSidebar.toggle() }
                        .help("Show or hide the Backlinks")
                }
            }
        }
        .overlay(alignment: .top) {
            if model.isShowingSwitcher {
                QuickSwitcher(model: model)
            }
        }
        .navigationTitle(model.shown?.name ?? model.vault?.root.lastPathComponent ?? "Noteki")
        .focusedSceneValue(\.noteEditor, model.isShowingNote ? model.editor : nil)
        .onAppear {
            // Files opened from the Finder bring this window back even after it was closed.
            registerShowMainWindow { openWindow(id: "main") }
            guard model.vault == nil else { return }
            Task {
                // A file opened at launch arrives just after this window appears.
                try? await Task.sleep(for: .milliseconds(400))
                if model.vault == nil, shouldAskForVault() { model.chooseVault() }
            }
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

private struct FilesPane: View {
    @Bindable var model: AppModel

    var body: some View {
        List(model.tree, children: \.children, selection: $model.selection) { item in
            Label(item.name, systemImage: item.symbolName)
        }
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
