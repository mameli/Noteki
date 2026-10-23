import NotekiCore
import SwiftUI

/// The left sidebar: Files, Tags and Search, chosen with a control at its top.
struct LeftSidebar: View {
    @Bindable var model: AppModel

    var body: some View {
        ZStack {
            // Files stays alive under the others: its open folders are kept.
            FilesPane(model: model)
                .opacity(model.leftPane == .files ? 1 : 0)
                .allowsHitTesting(model.leftPane == .files)
                .disabled(model.leftPane != .files)
                .accessibilityHidden(model.leftPane != .files)
            switch model.leftPane {
            case .files: EmptyView()
            case .tags: TagsPane(model: model)
            case .search: SearchPane(model: model)
            }
        }
        .safeAreaInset(edge: .top, spacing: 0) {
            Picker("Pane", selection: $model.leftPane) {
                Label("Files", systemImage: "folder").tag(AppModel.LeftPane.files)
                Label("Tags", systemImage: "number").tag(AppModel.LeftPane.tags)
                Label("Search", systemImage: "magnifyingglass").tag(AppModel.LeftPane.search)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .padding(.horizontal, 10)
            .padding(.bottom, 6)
        }
    }
}
