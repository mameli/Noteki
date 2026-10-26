import AppKit
import NotekiCore
import Observation

/// What the user chooses in Settings, kept in the app's preferences, outside the Vault (ADR
/// 0002): the editor's font, size and line width, and the folders each Vault excludes. Every
/// window's editor and model follow a change at once, told by a notification.
@MainActor
@Observable
final class AppSettings {
    static let shared = AppSettings()

    static let editorChanged = Notification.Name("NotekiEditorSettingsChanged")
    /// The Vault's root path is the object.
    static let excludedFoldersChanged = Notification.Name("NotekiExcludedFoldersChanged")

    private static let fontKey = "textFont"
    private static let sizeKey = "textSize"
    private static let lineWidthKey = "lineWidth"
    private static let excludedFoldersKey = "excludedFolders"

    var editor: EditorSettings {
        didSet {
            guard editor != oldValue else { return }
            let defaults = UserDefaults.standard
            defaults.set(editor.font, forKey: Self.fontKey)
            defaults.set(editor.size, forKey: Self.sizeKey)
            defaults.set(editor.lineWidth, forKey: Self.lineWidthKey)
            NotificationCenter.default.post(name: Self.editorChanged, object: nil)
        }
    }

    private init() {
        let defaults = UserDefaults.standard
        let standard = EditorSettings.standard
        editor = EditorSettings(
            font: defaults.string(forKey: Self.fontKey),
            size: defaults.object(forKey: Self.sizeKey) as? Double ?? standard.size,
            lineWidth: defaults.object(forKey: Self.lineWidthKey) as? Double ?? standard.lineWidth
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
