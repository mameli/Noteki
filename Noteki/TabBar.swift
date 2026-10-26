import NotekiCore
import SwiftUI

/// Noteki's own tab bar, above the Note (ADR 0003): a click brings a Tab in front, its × closes it.
struct TabBar: View {
    let model: AppModel

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 0) {
                ForEach(model.tabs.all) { tab in
                    TabButton(
                        title: tab.title,
                        isInFront: tab.id == model.tabs.front?.id,
                        select: { model.select(tab.id) },
                        close: { model.closeTab(tab.id) }
                    )
                    Divider().frame(height: 16)
                }
            }
        }
        .frame(height: 30)
        .background(.bar)
        .overlay(alignment: .bottom) { Divider() }
    }
}

private struct TabButton: View {
    let title: String
    let isInFront: Bool
    let select: () -> Void
    let close: () -> Void
    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 4) {
            Button(action: close) {
                Image(systemName: "xmark")
                    .font(.system(size: 9, weight: .semibold))
                    .frame(width: 16, height: 16)
                    .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .foregroundStyle(.secondary)
            // In front or under the pointer, as in Safari: a click elsewhere on the Tab selects it.
            .opacity(isInFront || isHovered ? 1 : 0)
            .help("Close Tab")
            .accessibilityLabel("Close \(title)")
            Text(title)
                .lineLimit(1)
                .truncationMode(.middle)
                .foregroundStyle(isInFront ? .primary : .secondary)
            Spacer(minLength: 0)
        }
        .font(.callout)
        .padding(.horizontal, 8)
        .frame(minWidth: 120, maxWidth: 220, maxHeight: .infinity)
        .background(isInFront ? AnyShapeStyle(.background) : AnyShapeStyle(.clear))
        .contentShape(.rect)
        .onTapGesture(perform: select)
        .onHover { isHovered = $0 }
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(isInFront ? [.isSelected, .isButton] : .isButton)
        .accessibilityLabel(title)
        .accessibilityAction(.default, select)
    }
}
