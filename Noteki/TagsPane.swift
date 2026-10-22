import NotekiCore
import SwiftUI

/// The left sidebar's choice between its panes, and the pane.
struct LeftSidebar: View {
    @Bindable var model: AppModel

    var body: some View {
        ZStack {
            // Files stays alive under the Tags: its open folders are kept.
            FilesPane(model: model)
                .opacity(model.leftPane == .files ? 1 : 0)
                .allowsHitTesting(model.leftPane == .files)
                .disabled(model.leftPane != .files)
                .accessibilityHidden(model.leftPane != .files)
            if model.leftPane == .tags {
                TagsPane(model: model)
            }
        }
        .safeAreaInset(edge: .top, spacing: 0) {
            Picker("Pane", selection: $model.leftPane) {
                Label("Files", systemImage: "folder").tag(AppModel.LeftPane.files)
                Label("Tags", systemImage: "number").tag(AppModel.LeftPane.tags)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .padding(.horizontal, 10)
            .padding(.bottom, 6)
        }
    }
}

/// The Tags of the Vault as a tree with the number of Notes, and the Notes of the Tag chosen.
private struct TagsPane: View {
    @Bindable var model: AppModel

    var body: some View {
        VStack(spacing: 0) {
            if model.tagTree.isEmpty {
                emptyState
            } else {
                List(model.tagTree, children: \.branches, selection: $model.selectedTag) { node in
                    HStack {
                        Label(node.name, systemImage: "number")
                        Spacer()
                        Text(node.noteCount, format: .number)
                            .foregroundStyle(.secondary)
                            .monospacedDigit()
                    }
                }
            }
            if model.selectedTag != nil {
                Divider()
                // Not a List: its table costs several MB for the few Notes a Tag has.
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(model.notesOfSelectedTag, id: \.path) { note in
                            Button { model.reveal(note.path) } label: {
                                HStack(spacing: 6) {
                                    Label(note.name, systemImage: "doc.text")
                                    if !note.folder.isEmpty {
                                        Text(note.folder)
                                            .foregroundStyle(.secondary)
                                            .lineLimit(1)
                                            .truncationMode(.head)
                                    }
                                }
                                .padding(.horizontal, 12)
                                .padding(.vertical, 4)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .contentShape(.rect)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.vertical, 4)
                }
                .frame(minHeight: 90, maxHeight: 260)
            }
        }
    }

    @ViewBuilder
    private var emptyState: some View {
        if model.isReadingTags {
            ContentUnavailableView("Reading the Tags of your Notes…", systemImage: "hourglass")
        } else {
            ContentUnavailableView(
                "No Tags", systemImage: "number",
                description: Text("Write #tag in a Note, or add a tags Property.")
            )
        }
    }
}

private extension TagNode {
    /// For the outline: nil for a Tag with nothing under it, so that it shows no disclosure triangle.
    var branches: [TagNode]? { children.isEmpty ? nil : children }
}
