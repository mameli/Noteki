import Foundation

/// A Note open for writing: the text the user sees and the last text Noteki saved or loaded,
/// the base an External change will be merged against (ADR 0005).
public struct OpenNote: Sendable {
    /// Follows the file when another app renames or moves it inside the Vault.
    public private(set) var note: Note
    /// What the editor shows, with `\n` between lines whatever the file uses.
    public private(set) var text: String
    private var base: String
    /// The file as Noteki last read or wrote it, to tell whether another app changed it since.
    private var onDisk: Data
    private let root: URL
    private var url: URL { root.appending(path: note.path, directoryHint: .notDirectory) }
    /// The file's identity on disk, the same across a rename: how a renamed Note is recognised.
    private var fileID: FileID?
    /// Set while the user's edits and the file's change touch the same lines: saving stops
    /// until the user keeps their version or takes the file's (ADR 0005).
    public private(set) var conflict: Conflict?
    /// Another app deleted the file (or moved it out of the Vault): the text stays, nothing is
    /// saved until the user saves it again.
    public private(set) var isDeleted = false
    private var theirsData: Data?
    /// Kept as most lines of the file had it, so saving never rewrites every line (and every
    /// line of a diff). The editor has one kind of line break: a mixed file comes out uniform.
    private var usesCRLF = false

    init(note: Note, root: URL) throws {
        self.note = note
        self.root = root
        onDisk = Data()
        text = ""
        base = ""
        take(try Data(contentsOf: url))
    }

    /// The file's content becomes what the editor shows and the base of the next save.
    private mutating func take(_ data: Data) {
        onDisk = data
        fileID = FileID(url)
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
        /// The file is gone: see `isDeleted`.
        case deleted
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
        // A case-only rename is reported under the new name: on a case-insensitive volume both
        // names are the same file.
        let reportedHere = paths.filter { $0.caseInsensitiveCompare(note.path) == .orderedSame }
        guard isDeleted || !reportedHere.isEmpty else { return .unchanged }
        let exists = FileManager.default.fileExists(atPath: url.path(percentEncoded: false))
        let sameFile = exists && fileID != nil && FileID(url) == fileID
        if !sameFile || isDeleted || reportedHere.contains(where: { $0 != note.path }) {
            if let renamed = paths.first(where: { isThisFile(at: $0) }) ?? (exists ? nil : renamedByContent(among: paths)) {
                // Renamed or moved, now or reported earlier: the same file under another path,
                // even if another file took the old one.
                note = Note(path: renamed)
            } else if !exists {
                isDeleted = true
                // The other version of a Conflict went with the file.
                conflict = nil
                theirsData = nil
                return .deleted
            }
            // Otherwise another file at the same path: a replacement, compared below.
            isDeleted = false
        }
        let data = try Data(contentsOf: url)
        guard data != onDisk else {
            // The other app put the file back as Noteki knew it, perhaps as a new file: no
            // Conflict any more, and the file followed is the one there now.
            conflict = nil
            theirsData = nil
            fileID = FileID(url)
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

    /// Noteki itself renamed or moved the Note, or a folder it is in, from `old` to `new`: it
    /// follows at once, without waiting for the file event.
    public mutating func moved(from old: String, to new: String) {
        if let path = pathAfterMove(note.path, from: old, to: new) { note = Note(path: path) }
    }

    /// Writes the text on screen where the deleted file was, and follows it again.
    public mutating func saveAgain() throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try write()
        isDeleted = false
        conflict = nil
        theirsData = nil
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
        /// The file was deleted: only `saveAgain` writes it.
        case deleted
        /// The file is not at its path: renamed or deleted, not noticed yet. Nothing was written;
        /// notice the change, then save again.
        case missing
    }

    /// The whole text after the user typed.
    public mutating func edit(_ newText: String) {
        text = newText
    }

    /// Writes the user's edits, and nothing when there are none. Never over a file another app
    /// changed since: then it throws `changedOnDisk` and the edits stay unsaved.
    public mutating func save() throws {
        guard hasUnsavedEdits else { return }
        guard !isDeleted else { throw SaveError.deleted }
        guard conflict == nil else { throw SaveError.conflict }
        guard FileManager.default.fileExists(atPath: url.path(percentEncoded: false)) else { throw SaveError.missing }
        guard try Data(contentsOf: url) == onDisk else { throw SaveError.changedOnDisk }
        try write()
    }

    private mutating func write() throws {
        let content = Data((usesCRLF ? text.replacingOccurrences(of: "\n", with: "\r\n") : text).utf8)
        try content.write(to: url, options: .atomic)
        onDisk = content
        // The atomic write put a new file in place: its identity is the one to follow now.
        fileID = FileID(url)
        base = text
    }

    /// An editor that saves by putting a new file in place, then renames it, in one report: the
    /// identity is new too. The Note is the one changed Note that keeps most of the lines Noteki
    /// knew, if exactly one does; otherwise none, rather than following the wrong file.
    private func renamedByContent(among paths: Set<String>) -> String? {
        let known = Set(base.split(separator: "\n").filter { !$0.isEmpty })
        guard !known.isEmpty else { return nil }
        let matches = paths.filter { path in
            guard path != note.path, hasNoteExtension(path),
                  let data = try? Data(contentsOf: root.appending(path: path, directoryHint: .notDirectory))
            else { return false }
            let lines = Set(Self.editorText(of: data).text.split(separator: "\n"))
            return Double(known.intersection(lines).count) >= Double(known.count) * 0.6
        }
        return matches.count == 1 ? matches.first : nil
    }

    /// `.md` for a Note; for a text file opened on its own (a Loose file), its own extension.
    private func hasNoteExtension(_ path: String) -> Bool {
        (path as NSString).pathExtension == (note.path as NSString).pathExtension
    }

    private func isThisFile(at path: String) -> Bool {
        guard path != note.path, hasNoteExtension(path), let fileID else { return false }
        return FileID(root.appending(path: path, directoryHint: .notDirectory)) == fileID
    }
}

extension Vault {
    /// Opens a Note for writing.
    public func open(_ note: Note) throws -> OpenNote {
        try OpenNote(note: note, root: root)
    }
}

/// A file's device and inode: unchanged by a rename or a move on the same volume, new when an
/// editor saves by putting a new file in place.
private struct FileID: Equatable, Sendable {
    let device: UInt64
    let inode: UInt64

    init?(_ url: URL) {
        var info = stat()
        guard stat(url.path(percentEncoded: false), &info) == 0 else { return nil }
        device = UInt64(info.st_dev)
        inode = UInt64(info.st_ino)
    }
}
