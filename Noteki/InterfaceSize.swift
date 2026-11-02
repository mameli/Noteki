import SwiftUI

/// How large the sidebars, the Tabs and the panes are drawn, chosen in Settings; the Note's text
/// has a size of its own. Small is the system's own sizes, as Noteki drew them until 0.1.0; Medium is the default.
enum InterfaceSize: String, CaseIterable, Identifiable {
    case small, medium, large

    var id: Self { self }

    var name: String {
        switch self {
        case .small: "Small"
        case .medium: "Medium"
        case .large: "Large"
        }
    }

    /// Points added to every text style: macOS draws its styles at one size only.
    var extra: CGFloat {
        switch self {
        case .small: 0
        case .medium: 2
        case .large: 4
        }
    }

    /// The Files rows: compact as Mail's at Small.
    var rowSize: SidebarRowSize {
        switch self {
        case .small: .small
        case .medium: .medium
        case .large: .large
        }
    }

    /// `style` at this size; the system's own at Small.
    func font(_ style: Font.TextStyle, weight: Font.Weight? = nil) -> Font {
        guard extra > 0 else { return weight.map { .system(style).weight($0) } ?? .system(style) }
        return .system(size: Self.points[style, default: 13] + extra, weight: weight ?? (style == .headline ? .bold : .regular))
    }

    /// The sizes of macOS's text styles.
    private static let points: [Font.TextStyle: CGFloat] = [
        .largeTitle: 26, .title: 22, .title2: 17, .title3: 15, .headline: 13, .body: 13,
        .callout: 12, .subheadline: 11, .footnote: 10, .caption: 10, .caption2: 10,
    ]
}

extension EnvironmentValues {
    @Entry var interfaceSize: InterfaceSize = .small
}

extension View {
    /// The window at the user's Interface size: text without a style of its own is body text.
    func interfaceSize(_ size: InterfaceSize) -> some View {
        environment(\.interfaceSize, size)
            .font(size.extra > 0 ? size.font(.body) : nil)
    }

    /// `style` at the window's Interface size.
    func interfaceFont(_ style: Font.TextStyle, weight: Font.Weight? = nil) -> some View {
        modifier(InterfaceFont(style: style, weight: weight))
    }
}

private struct InterfaceFont: ViewModifier {
    let style: Font.TextStyle
    let weight: Font.Weight?
    @Environment(\.interfaceSize) private var size

    func body(content: Content) -> some View {
        content.font(size.font(style, weight: weight))
    }
}
