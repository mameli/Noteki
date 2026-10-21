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
    private let usesCRLF: Bool

    init(note: Note, url: URL) throws {
        self.note = note
        self.url = url
        onDisk = try Data(contentsOf: url)
        let content = String(decoding: onDisk, as: UTF8.self)
        let lineBreaks = content.utf8.count { $0 == UInt8(ascii: "\n") }
        let crlfBreaks = content.components(separatedBy: "\r\n").count - 1
        usesCRLF = crlfBreaks * 2 > lineBreaks
        text = usesCRLF ? content.replacingOccurrences(of: "\r\n", with: "\n") : content
        base = text
    }

    public var hasUnsavedEdits: Bool { text != base }

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
