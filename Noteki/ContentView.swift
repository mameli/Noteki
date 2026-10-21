import NotekiCore
import SwiftUI

struct ContentView: View {
    @Bindable var model: AppModel

    var body: some View {
        NavigationSplitView {
            FilesPane(model: model)
                .navigationSplitViewColumnWidth(min: 200, ideal: 260)
        } detail: {
            detail
        }
        .navigationTitle(model.shown?.name ?? model.vault?.root.lastPathComponent ?? "Noteki")
        .focusedSceneValue(\.noteEditor, model.isShowingNote ? model.editor : nil)
        .onAppear {
            if model.vault == nil { model.chooseVault() }
        }
    }

    @ViewBuilder
    private var detail: some View {
        if model.vault == nil {
            ContentUnavailableView {
                Label("No Vault", systemImage: "folder")
            } description: {
                Text(AppModel.chooseVaultMessage)
            } actions: {
                Button("Choose Vault…") { model.chooseVault() }
            }
        } else {
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
                    if let saveFailure = model.saveFailure {
                        Label(saveFailure, systemImage: "exclamationmark.triangle.fill")
                            .padding(.horizontal, 12)
                            .padding(.vertical, 8)
                            .background(.red.opacity(0.9), in: .rect(cornerRadius: 8))
                            .foregroundStyle(.white)
                            .padding()
                    }
                }
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
