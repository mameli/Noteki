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
    /// Set while the user's edits and the file's change touch the same lines: saving stops
    /// until the user keeps their version or takes the file's (ADR 0005).
    public private(set) var conflict: Conflict?
    private var theirsData: Data?
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
        /// Show this text, then `apply` it: the file's new content when nothing was unsaved, or
        /// the user's edits merged with it. Until applied the open Note keeps the old base, so
        /// text typed on the old content cannot overwrite the new.
        case reload(Reload)
        /// The user's edits and the change touch the same lines: see `conflict`. The editor keeps
        /// the user's text and nothing is saved until the user chooses.
        case conflict
    }

    /// The two versions of a Conflict, for the user to compare.
    public struct Conflict: Equatable, Sendable {
        public let mine: String
        public let theirs: String

        public init(mine: String, theirs: String) {
            self.mine = mine
            self.theirs = theirs
        }
    }

    /// The file's new content, or the user's edits merged with it, waiting for the editor.
    public struct Reload: Equatable, Sendable {
        /// The file as it is now: the base of the next save once applied.
        fileprivate let data: Data
        /// The user's text this was computed from: if it changed since, it no longer applies.
        fileprivate let mine: String
        /// What the editor should show, with `\n` between lines.
        public let text: String
    }

    /// The Vault's change entry for the open Note: the paths the Vault reported as changed on
    /// disk (`Vault.changedPaths`). Reads the file only when its own path is among them.
    public mutating func noticeChanges(at paths: Set<String>) throws -> ExternalChange {
        guard paths.contains(note.path) else { return .unchanged }
        let data = try Data(contentsOf: url)
        guard data != onDisk else {
            // The other app put the file back as Noteki knew it: no Conflict any more.
            conflict = nil
            theirsData = nil
            return .unchanged
        }
        guard data != theirsData || conflict == nil else { return .unchanged }
        let theirs = Self.editorText(of: data).text
        guard hasUnsavedEdits else { return .reload(Reload(data: data, mine: text, text: theirs)) }
        guard let merged = Merge.merge(base: base, mine: text, theirs: theirs) else {
            conflict = Conflict(mine: text, theirs: theirs)
            theirsData = data
            return .conflict
        }
        return .reload(Reload(data: data, mine: text, text: merged))
    }

    /// Ends a Conflict with the user's text: written over the file, erasing the other change.
    public mutating func keepMine() throws {
        try write()
        conflict = nil
        theirsData = nil
    }

    /// Ends a Conflict with the file's text: the user's edits are dropped. The editor shows `text`.
    public mutating func takeTheirs() {
        guard let theirsData else { return }
        take(theirsData)
        conflict = nil
        self.theirsData = nil
    }

    /// The editor shows the reload: its text becomes the open Note's, and the file's content the
    /// base of the next save, which writes what was merged. False, changing nothing, when the
    /// user's text changed since the reload was computed: notice the change again.
    @discardableResult
    public mutating func apply(_ reload: Reload) -> Bool {
        guard text == reload.mine else { return false }
        take(reload.data)
        text = reload.text
        conflict = nil
        theirsData = nil
        return true
    }

    public enum SaveError: Error, Equatable {
        /// Another app wrote the file since Noteki last read or saved it. Saving would erase that
        /// change: notice it first, to merge it (ADR 0005).
        case changedOnDisk
        /// A Conflict waits for the user's choice.
        case conflict
    }

    /// The whole text after the user typed.
    public mutating func edit(_ newText: String) {
        text = newText
    }

    /// Writes the user's edits, and nothing when there are none. Never over a file another app
    /// changed since: then it throws `changedOnDisk` and the edits stay unsaved.
    public mutating func save() throws {
        guard hasUnsavedEdits else { return }
        guard conflict == nil else { throw SaveError.conflict }
        guard try Data(contentsOf: url) == onDisk else { throw SaveError.changedOnDisk }
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
