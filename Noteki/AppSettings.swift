import AppKit
import NotekiCore
import Observation

/// What the user chooses in Settings, kept in the app's preferences, outside the Vault (ADR
/// 0002): the size of the window's sidebars and Tabs, the editor's font, size and line width, the folders each Vault excludes, and where
/// the Daily note is. Every window's editor and model follow a change at once, told by a
/// notification; the Daily note's place is read when ⌘⇧D is pressed.
@MainActor
@Observable
final class AppSettings {
    static let shared = AppSettings()

    static let editorChanged = Notification.Name("NotekiEditorSettingsChanged")
    /// The Vault's root path is the object.
    static let excludedFoldersChanged = Notification.Name("NotekiExcludedFoldersChanged")

    private static let themeKey = "theme"
    private static let fontKey = "textFont"
    private static let sizeKey = "textSize"
    private static let monoFontKey = "codeFont"
    private static let lineLengthKey = "lineLength"
    private static let limitsLineLengthKey = "limitsLineLength"
    private static let checksSpellingKey = "checksSpelling"
    private static let correctsSpellingKey = "correctsSpelling"
    /// In points, until 0.1.0: read once, as a length.
    private static let lineWidthKey = "lineWidth"
    private static let excludedFoldersKey = "excludedFolders"
    private static let dailyFolderKey = "dailyNoteFolder"
    private static let dailyFormatKey = "dailyNoteFormat"
    private static let formatBarKey = "showsFormatBar"
    private static let interfaceSizeKey = "interfaceSize"
    private static let appearanceKey = "appearance"

    /// Light, dark or the Mac's: set on the app, which the editor's page and the colours follow.
    var appearance = Appearance(rawValue: UserDefaults.standard.string(forKey: AppSettings.appearanceKey) ?? "") ?? .system {
        didSet {
            UserDefaults.standard.set(appearance.rawValue, forKey: Self.appearanceKey)
            applyAppearance()
        }
    }

    func applyAppearance() {
        NSApplication.shared.appearance = appearance.nsAppearance
    }

    /// How large the sidebars, the Tabs and the panes are drawn; the Note's text has its own size.
    var interfaceSize = InterfaceSize(rawValue: UserDefaults.standard.string(forKey: AppSettings.interfaceSizeKey) ?? "") ?? .medium {
        didSet { UserDefaults.standard.set(interfaceSize.rawValue, forKey: Self.interfaceSizeKey) }
    }

    /// The row of Format buttons over the Note; hidden at first, for those who use the shortcuts.
    var showsFormatBar = UserDefaults.standard.bool(forKey: AppSettings.formatBarKey) {
        didSet { UserDefaults.standard.set(showsFormatBar, forKey: Self.formatBarKey) }
    }

    /// The Daily note's folder from the Vault's root and its date format: one for the app, as
    /// Noteki is used with one Vault, where the Excluded folders are each Vault's own.
    var dailyNote: DailyNote {
        didSet {
            UserDefaults.standard.set(dailyNote.folder, forKey: Self.dailyFolderKey)
            UserDefaults.standard.set(dailyNote.format, forKey: Self.dailyFormatKey)
        }
    }

    var editor: EditorSettings {
        didSet {
            guard editor != oldValue else { return }
            let defaults = UserDefaults.standard
            defaults.set(editor.theme.rawValue, forKey: Self.themeKey)
            defaults.set(editor.font, forKey: Self.fontKey)
            defaults.set(editor.monoFont, forKey: Self.monoFontKey)
            defaults.set(editor.limitsLineLength, forKey: Self.limitsLineLengthKey)
            defaults.set(editor.checksSpelling, forKey: Self.checksSpellingKey)
            defaults.set(editor.correctsSpelling, forKey: Self.correctsSpellingKey)
            // WebKit's own, read by every web view: the page's attributes alone may not stop the Mac
            // correcting as typed.
            defaults.set(editor.checksSpelling, forKey: "WebContinuousSpellCheckingEnabled")
            defaults.set(editor.correctsSpelling, forKey: "WebAutomaticSpellingCorrectionEnabled")
            defaults.set(editor.size, forKey: Self.sizeKey)
            defaults.set(editor.lineLength, forKey: Self.lineLengthKey)
            defaults.removeObject(forKey: Self.lineWidthKey)
            NotificationCenter.default.post(name: Self.editorChanged, object: nil)
        }
    }

    private init() {
        let defaults = UserDefaults.standard
        let standard = EditorSettings.standard
        let size = defaults.object(forKey: Self.sizeKey) as? Double ?? standard.size
        let oldWidth = (defaults.object(forKey: Self.lineWidthKey) as? Double).map { EditorSettings.lineLength(width: $0, size: size) }
        editor = EditorSettings(
            theme: Theme(rawValue: defaults.string(forKey: Self.themeKey) ?? "") ?? standard.theme,
            font: defaults.string(forKey: Self.fontKey),
            monoFont: defaults.string(forKey: Self.monoFontKey),
            size: size,
            lineLength: defaults.object(forKey: Self.lineLengthKey) as? Double ?? oldWidth ?? standard.lineLength,
            limitsLineLength: defaults.object(forKey: Self.limitsLineLengthKey) as? Bool ?? standard.limitsLineLength,
            checksSpelling: defaults.object(forKey: Self.checksSpellingKey) as? Bool ?? standard.checksSpelling,
            correctsSpelling: defaults.object(forKey: Self.correctsSpellingKey) as? Bool ?? standard.correctsSpelling
        )
        dailyNote = DailyNote(
            folder: defaults.string(forKey: Self.dailyFolderKey) ?? DailyNote.standard.folder,
            format: defaults.string(forKey: Self.dailyFormatKey) ?? DailyNote.standard.format
        )
    }

    /// The folders the user excludes from the Vault at `root`, by their paths from it.
    func excludedFolders(of root: URL) -> [String] {
        (UserDefaults.standard.dictionary(forKey: Self.excludedFoldersKey)?[root.path(percentEncoded: false)] as? [String]) ?? []
    }

    func setExcludedFolders(_ folders: [String], of root: URL) {
        var all = UserDefaults.standard.dictionary(forKey: Self.excludedFoldersKey) ?? [:]
        all[root.path(percentEncoded: false)] = folders.isEmpty ? nil : folders
        UserDefaults.standard.set(all, forKey: Self.excludedFoldersKey)
        NotificationCenter.default.post(name: Self.excludedFoldersChanged, object: root.path(percentEncoded: false))
    }
}
