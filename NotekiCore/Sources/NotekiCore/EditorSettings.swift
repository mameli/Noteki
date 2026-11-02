import Foundation

/// The colours Noteki draws with, each light and dark (SPEC, Window): the editor's page has them
/// in editor.css, under the same name.
public enum Theme: String, CaseIterable, Sendable, Encodable {
    case github
    case anuppuccin
    case oneDarkPro = "one-dark-pro"

    public var name: String {
        switch self {
        case .github: "GitHub"
        case .anuppuccin: "AnuPpuccin"
        case .oneDarkPro: "One Dark Pro"
        }
    }

    /// Its accent as hex, light and dark: links in the text, icons and the selected pane outside it.
    public var accent: (light: String, dark: String) {
        switch self {
        case .github: ("#0969DA", "#4493F8")
        case .anuppuccin: ("#D6817D", "#F5E0DC")
        case .oneDarkPro: ("#4078F2", "#61AFEF")
        }
    }

    /// Its panels' colour as hex, light and dark, a step off the background: the sidebars, as the
    /// page's `--panel`.
    public var panel: (light: String, dark: String) {
        switch self {
        case .github: ("#F6F8FA", "#151B23")
        case .anuppuccin: ("#E6DBD1", "#181825")
        case .oneDarkPro: ("#F0F0F1", "#21252B")
        }
    }

    /// Its background as hex, light and dark: behind the Tabs as behind the text.
    public var background: (light: String, dark: String) {
        switch self {
        case .github: ("#FFFFFF", "#0D1117")
        case .anuppuccin: ("#EEE6DD", "#1E1E2E")
        case .oneDarkPro: ("#FAFAFA", "#282C34")
        }
    }
}

/// How the editor draws a Note's text, chosen in Settings (SPEC, Window): the theme, the font and the code's,
/// the size in points (a CSS pixel of the editor is a point) and the longest a line is, in
/// characters, if at all. Kept outside the Vault, in the app's preferences. Spelling is WebKit's
/// own setting, not one of these.
public struct EditorSettings: Equatable, Sendable, Encodable {
    /// The app's, which the page draws with.
    public let theme: Theme
    /// A font family's name; nil for the system font.
    public let font: String?
    /// The font of code, inline and in blocks; nil for the system's monospaced font.
    public let monoFont: String?
    public let size: Double
    /// In characters of half an em, a letter's width in text on average (the system font's is
    /// 0.49 em): it follows the size, and zoom. Not CSS's `ch`, a digit's width, a third wider.
    public let lineLength: Double
    /// False: lines as wide as the window, `lineLength` kept for when it is on again.
    public let limitsLineLength: Bool

    public static let sizes: ClosedRange<Double> = 10...32
    public static let lineLengths: ClosedRange<Double> = 40...200
    /// The system font at 15 points, lines of about 100 characters: as wide as before (736 points).
    public static let standard = EditorSettings(font: nil, size: 15, lineLength: 100)

    /// Sizes are whole points and lengths whole characters, within their bounds; a blank font is the system's.
    public init(
        theme: Theme = .github, font: String?, monoFont: String? = nil, size: Double, lineLength: Double, limitsLineLength: Bool = true
    ) {
        self.theme = theme
        self.font = Self.family(font)
        self.monoFont = Self.family(monoFont)
        self.size = min(max(size.rounded(), Self.sizes.lowerBound), Self.sizes.upperBound)
        self.lineLength = min(max(lineLength.rounded(), Self.lineLengths.lowerBound), Self.lineLengths.upperBound)
        self.limitsLineLength = limitsLineLength
    }

    private static func family(_ name: String?) -> String? {
        let name = name?.trimmingCharacters(in: .whitespaces)
        return name?.isEmpty == false ? name : nil
    }

    /// The line length of a width in points chosen at `size`, as Settings kept it until 0.1.0.
    public static func lineLength(width: Double, size: Double) -> Double {
        width / (size * 0.5)
    }

    /// The same, with what is given changed.
    public func with(
        theme: Theme? = nil, font: String?? = nil, monoFont: String?? = nil, size: Double? = nil, lineLength: Double? = nil,
        limitsLineLength: Bool? = nil
    ) -> EditorSettings {
        EditorSettings(
            theme: theme ?? self.theme, font: font ?? self.font, monoFont: monoFont ?? self.monoFont, size: size ?? self.size,
            lineLength: lineLength ?? self.lineLength, limitsLineLength: limitsLineLength ?? self.limitsLineLength
        )
    }

    /// As the editor reads it, before it loads and with every change.
    public func json() throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        return String(decoding: try encoder.encode(self), as: UTF8.self)
    }
}
