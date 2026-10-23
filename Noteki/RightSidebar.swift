import NotekiCore
import SwiftUI

/// The right sidebar: the Outline and the Backlinks of the Note in front.
struct RightSidebar: View {
    @Bindable var model: AppModel

    var body: some View {
        Group {
            switch model.rightPane {
            case .outline: OutlinePane(model: model)
            case .backlinks: BacklinksPane(model: model)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .safeAreaInset(edge: .top, spacing: 0) {
            Picker("Pane", selection: $model.rightPane) {
                Label("Outline", systemImage: "list.bullet.indent").tag(AppModel.RightPane.outline)
                Label("Backlinks", systemImage: "link").tag(AppModel.RightPane.backlinks)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
        }
    }
}

/// The Headings of the Note in front, indented by level; a click moves the editor to one.
private struct OutlinePane: View {
    let model: AppModel

    var body: some View {
        // Not a List: its table costs several MB for a few Headings.
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                if model.outline.isEmpty {
                    emptyState
                        .foregroundStyle(.secondary)
                        .padding(12)
                }
                ForEach(model.outline, id: \.line) { heading in
                    Button { model.go(to: heading) } label: {
                        Text(heading.text)
                            .fontWeight(heading.level == 1 ? .semibold : .regular)
                            .lineLimit(2)
                            .padding(.leading, 12 + CGFloat(heading.level - 1) * 14)
                            .padding(.trailing, 12)
                            .padding(.vertical, 3)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.vertical, 4)
        }
    }

    private var emptyState: some View {
        Label(model.isShowingNote ? "No Headings in this Note." : "No Note in front.", systemImage: "list.bullet.indent")
    }
}
