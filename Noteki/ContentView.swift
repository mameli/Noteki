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
        .navigationTitle(model.note?.name ?? model.vault?.root.lastPathComponent ?? "Noteki")
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
            EditorView(text: model.noteText)
                .overlay {
                    if let failure = model.failure {
                        placeholder(ContentUnavailableView(
                            "Cannot Open", systemImage: "exclamationmark.triangle", description: Text(failure)
                        ))
                    } else if model.note == nil {
                        placeholder(ContentUnavailableView("No Note Selected", systemImage: "doc.text"))
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
            Label(item.name, systemImage: item.children == nil ? "doc.text" : "folder")
        }
    }
}

extension VaultItem: @retroactive Identifiable {
    public var id: String { path }

    /// For the Files pane's outline: nil for Notes, so they show no disclosure triangle.
    var children: [VaultItem]? {
        switch self {
        case .folder(_, let children): children
        case .note: nil
        }
    }
}
