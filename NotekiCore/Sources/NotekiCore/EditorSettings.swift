import Foundation

/// How the editor draws a Note's text, chosen in Settings (SPEC, Window): the font, its size in
/// points and the widest a line is, in points (a CSS pixel of the editor is a point). Kept
/// outside the Vault, in the app's preferences.
public struct EditorSettings: Equatable, Sendable, Encodable {
    /// A font family's name; nil for the system font.
    public let font: String?
    public let size: Double
    public let lineWidth: Double

    public static let sizes: ClosedRange<Double> = 10...32
    public static let lineWidths: ClosedRange<Double> = 480...1600
    /// The system font at 15 points, lines at most 46 rem wide: as before there were Settings.
    public static let standard = EditorSettings(font: nil, size: 15, lineWidth: 736)

    /// Sizes and widths are whole points within their bounds; a blank font is the system's.
    public init(font: String?, size: Double, lineWidth: Double) {
        let font = font?.trimmingCharacters(in: .whitespaces)
        self.font = font?.isEmpty == false ? font : nil
        self.size = min(max(size.rounded(), Self.sizes.lowerBound), Self.sizes.upperBound)
        self.lineWidth = min(max(lineWidth.rounded(), Self.lineWidths.lowerBound), Self.lineWidths.upperBound)
    }

    /// The same, with what is given changed.
    public func with(font: String?? = nil, size: Double? = nil, lineWidth: Double? = nil) -> EditorSettings {
        EditorSettings(font: font ?? self.font, size: size ?? self.size, lineWidth: lineWidth ?? self.lineWidth)
    }

    /// As the editor reads it, before it loads and with every change.
    public func json() throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        return String(decoding: try encoder.encode(self), as: UTF8.self)
    }
}
