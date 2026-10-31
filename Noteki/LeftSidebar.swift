import NotekiCore
import SwiftUI

/// The left sidebar: Files, Tags and Search, chosen with icons in its part of the toolbar.
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
        .toolbar {
            ToolbarItem {
                PaneSwitcher(selection: $model.leftPane, panes: [
                    .init(.files, "Files (⌘1)", "folder"),
                    .init(.tags, "Tags (⌘2)", "number"),
                    .init(.search, "Search (⌘3)", "magnifyingglass"),
                ])
            }
        }
    }
}

/// A row of icons in a sidebar's part of the toolbar, as Xcode's navigators: the pane shown is in the
/// accent colour, each has its name as a tooltip.
struct PaneSwitcher<Pane: Hashable>: View {
    struct Item {
        let pane: Pane
        let name: String
        let symbol: String
        var badge: Int? = nil
        init(_ pane: Pane, _ name: String, _ symbol: String, badge: Int? = nil) {
            (self.pane, self.name, self.symbol, self.badge) = (pane, name, symbol, badge)
        }
    }

    @Binding var selection: Pane
    let panes: [Item]

    var body: some View {
        HStack(spacing: 4) {
            ForEach(panes, id: \.pane) { item in
                Button { selection = item.pane } label: {
                    Image(systemName: item.symbol)
                        .symbolVariant(selection == item.pane ? .fill : .none)
                        .foregroundStyle(selection == item.pane ? AnyShapeStyle(.tint) : AnyShapeStyle(.secondary))
                        .frame(width: 28, height: 22)
                        .overlay(alignment: .topTrailing) {
                            if let badge = item.badge, badge > 0 {
                                Text("\(badge)")
                                    .font(.system(size: 9, weight: .semibold).monospacedDigit())
                                    .foregroundStyle(.secondary)
                                    .offset(x: 2, y: -3)
                            }
                        }
                        .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .help(item.name)
                .accessibilityLabel(item.name)
                .accessibilityAddTraits(selection == item.pane ? .isSelected : [])
            }
        }
        .imageScale(.medium)
    }
}
