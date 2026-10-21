import Foundation

/// A Note open for writing: the text the user sees and the last text Noteki saved or loaded,
/// the base an External change will be merged against (ADR 0005).
public struct OpenNote: Sendable {
    public let note: Note
    /// What the editor shows, with `\n` between lines whatever the file uses.
    public private(set) var text: String
    private var base: String
    /// The file as Noteki last read or wrote it, to tell whether another app changed it since.
    private var onDisk: Data
    private let url: URL
    /// Kept as most lines of the file had it, so saving never rewrites every line (and every
    /// line of a diff). The editor has one kind of line break: a mixed file comes out uniform.
    private var usesCRLF = false

    init(note: Note, url: URL) throws {
        self.note = note
        self.url = url
        onDisk = Data()
        text = ""
        base = ""
        take(try Data(contentsOf: url))
    }

    /// The file's content becomes what the editor shows and the base of the next save.
    private mutating func take(_ data: Data) {
        onDisk = data
        (text, usesCRLF) = Self.editorText(of: data)
        base = text
    }

    /// The file's text with `\n` between lines, and whether most of its lines ended with CRLF.
    private static func editorText(of data: Data) -> (text: String, usesCRLF: Bool) {
        let content = String(decoding: data, as: UTF8.self)
        let lineBreaks = content.utf8.count { $0 == UInt8(ascii: "\n") }
        let crlfBreaks = content.components(separatedBy: "\r\n").count - 1
        let usesCRLF = crlfBreaks * 2 > lineBreaks
        return (usesCRLF ? content.replacingOccurrences(of: "\r\n", with: "\n") : content, usesCRLF)
    }

    public var hasUnsavedEdits: Bool { text != base }

    /// What an External change means for the open Note.
    public enum ExternalChange: Equatable, Sendable {
        /// Not this Note, or the file is as Noteki last read or wrote it (its own save).
        case unchanged
        /// Nothing was unsaved: show the new content, then `apply` it. Until applied the open
        /// Note keeps the old base, so text typed on the old content cannot overwrite the new.
        case reload(Reload)
        /// The user's edits are kept and the file left alone: saving refuses until the two are
        /// merged (ticket 07) or the user chooses.
        case keptUnsavedEdits
    }

    /// The file's new content, waiting for the editor to show it.
    public struct Reload: Equatable, Sendable {
        fileprivate let data: Data
        /// What the editor should show, with `\n` between lines.
        public let text: String
    }

    /// The Vault's change entry for the open Note: the paths the Vault reported as changed on
    /// disk (`Vault.changedPaths`). Reads the file only when its own path is among them.
    public func noticeChanges(at paths: Set<String>) throws -> ExternalChange {
        guard paths.contains(note.path) else { return .unchanged }
        let data = try Data(contentsOf: url)
        guard data != onDisk else { return .unchanged }
        guard !hasUnsavedEdits else { return .keptUnsavedEdits }
        return .reload(Reload(data: data, text: Self.editorText(of: data).text))
    }

    /// The editor shows the reloaded content: it becomes the text and the base of the next save.
    public mutating func apply(_ reload: Reload) {
        take(reload.data)
    }

    public enum SaveError: Error, Equatable {
        /// Another app wrote the file since Noteki last read or saved it. Saving would erase that
        /// change; merging it is the job of External change handling (ADR 0005).
        case changedOnDisk
    }

    /// The whole text after the user typed.
    public mutating func edit(_ newText: String) {
        text = newText
    }

    /// Writes the user's edits, and nothing when there are none. Never over a file another app
    /// changed since: then it throws `changedOnDisk` and the edits stay unsaved.
    public mutating func save() throws {
        guard hasUnsavedEdits else { return }
        guard try Data(contentsOf: url) == onDisk else { throw SaveError.changedOnDisk }
        try write()
    }

    /// Writes the user's edits even over another app's change, erasing it: only when the user
    /// chose their version.
    public mutating func saveReplacingChangesOnDisk() throws {
        try write()
    }

    private mutating func write() throws {
        let content = Data((usesCRLF ? text.replacingOccurrences(of: "\n", with: "\r\n") : text).utf8)
        try content.write(to: url, options: .atomic)
        onDisk = content
        base = text
    }
}

extension Vault {
    /// Opens a Note for writing.
    public func open(_ note: Note) throws -> OpenNote {
        try OpenNote(note: note, url: url(of: note))
    }
}
