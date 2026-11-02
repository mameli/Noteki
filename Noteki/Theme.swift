import AppKit
import NotekiCore
import SwiftUI

extension Theme {
    /// Its background outside the editor's page, behind the Tabs, the placeholders and an image, so
    /// the Note's column is one colour from the toolbar down: light or dark as the view is drawn.
    var backgroundColor: NSColor { Self.dynamic(background) }

    /// Its accent outside the page: the Files icons, the selected pane, the Quick switcher's row.
    var accentColor: NSColor { Self.dynamic(accent) }

    /// Its panels: both sidebars and the Quick switcher, solid, not the system's translucent ones.
    var panelColor: NSColor { Self.dynamic(panel) }

    private static func dynamic(_ hex: (light: String, dark: String)) -> NSColor {
        let (light, dark) = (NSColor(hex: hex.light), NSColor(hex: hex.dark))
        return NSColor(name: nil) { $0.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark : light }
    }
}

extension Color {
    /// The theme's background, as Settings say now.
    @MainActor static var editorBackground: Color {
        Color(nsColor: AppSettings.shared.editor.theme.backgroundColor)
    }

    /// The theme's panels, as Settings say now.
    @MainActor static var themePanel: Color {
        Color(nsColor: AppSettings.shared.editor.theme.panelColor)
    }

    /// The theme's accent, as Settings say now.
    @MainActor static var themeAccent: Color {
        Color(nsColor: AppSettings.shared.editor.theme.accentColor)
    }
}

extension View {
    /// A sidebar in the theme's panel colour, up through the toolbar: its lists drawn on it, with
    /// no background of their own.
    func themePanel() -> some View {
        scrollContentBackground(.hidden)
            .background(Color.themePanel.ignoresSafeArea())
    }
}

private extension NSColor {
    /// `#RRGGBB`.
    convenience init(hex: String) {
        let value = UInt32(hex.dropFirst(), radix: 16) ?? 0
        self.init(
            srgbRed: CGFloat((value >> 16) & 0xFF) / 255, green: CGFloat((value >> 8) & 0xFF) / 255,
            blue: CGFloat(value & 0xFF) / 255, alpha: 1
        )
    }
}

/// Light, dark, or as the Mac is, chosen in Settings: the whole app, its editor's page with it.
enum Appearance: String, CaseIterable, Identifiable {
    case system, light, dark

    var id: Self { self }

    var name: String {
        switch self {
        case .system: "System"
        case .light: "Light"
        case .dark: "Dark"
        }
    }

    /// The app's appearance; nil follows the Mac's.
    var nsAppearance: NSAppearance? {
        switch self {
        case .system: nil
        case .light: NSAppearance(named: .aqua)
        case .dark: NSAppearance(named: .darkAqua)
        }
    }
}
