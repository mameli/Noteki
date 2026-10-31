import NotekiCore
import SwiftUI

/// The right sidebar: the Outline and the Backlinks of the Note in front, chosen with icons
/// at the toolbar's trailing end.
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
                            .font(.callout)
                            .foregroundStyle(heading.level == 1 ? .primary : .secondary)
                            .lineLimit(2)
                            .padding(.leading, 12 + CGFloat(heading.level - 1) * 12)
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
