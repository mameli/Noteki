import Foundation

// What the Files pane and ⌘N do to the Vault (SPEC, Files): create, rename, move, move to the
// Trash. Never over an existing file, never deleted for good. Wikilinks to a renamed Note are not
// updated in v1.
extension Vault {
    public enum FileError: Error, Equatable {
        /// The name is taken by this file or folder (perhaps in other letter case).
        case exists(String)
        /// Empty, with a character Obsidian refuses on a Mac (`/`, `\\`, `:`) or one a Wikilink
        /// cannot hold (`[`, `]`, `#`, `^`, `|`), or a name the Vault does not list: starting
        /// with a dot, or an Excluded folder's (`node_modules`).
        case invalidName
        /// A folder moved into itself, or into a folder inside it.
        case intoItself
    }

    /// An empty `Untitled.md`, or `Untitled 1.md`…, in `folder` ("" for the root).
    public func newNote(in folder: String) throws -> Note {
        Note(path: try writeNew(as: "Untitled", extension: Note.fileExtension, in: folder) { file in
            try Data().write(to: file, options: .withoutOverwriting)
        })
    }

    /// An empty folder `Untitled`, or `Untitled 1`…, in `folder`, by its path.
    public func newFolder(in folder: String) throws -> String {
        try writeNew(as: "Untitled", extension: nil, in: folder) { file in
            try FileManager.default.createDirectory(at: file, withIntermediateDirectories: false)
        }
    }

    /// Gives the Note, Attachment or folder at `path` a new `name`, in the same folder; its new
    /// path. A Note's name is without `.md` and an Attachment's without its extension: both keep
    /// theirs (typed again, it is not doubled).
    public func rename(_ path: String, to name: String) throws -> String {
        let name = name.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty, !name.hasPrefix("."), !name.contains(where: { "/\\:[]#^|".contains($0) }) else { throw FileError.invalidName }
        var isFolder: ObjCBool = false
        FileManager.default.fileExists(atPath: fileURL(path).path(percentEncoded: false), isDirectory: &isFolder)
        var fileName = name
        let fileExtension = (path as NSString).pathExtension
        if !isFolder.boolValue, !fileExtension.isEmpty {
            if (name as NSString).pathExtension.lowercased() == fileExtension.lowercased() {
                fileName = (name as NSString).deletingPathExtension
            }
            fileName += "." + fileExtension
        }
        let folder = folderOf(path)
        let newPath = folder.isEmpty ? fileName : folder + "/" + fileName
        // `node_modules`, `venv`: the Vault would no longer list it.
        guard !isInExcludedFolder(newPath) else { throw FileError.invalidName }
        return try moveItem(path, to: newPath)
    }

    /// Moves the Note, Attachment or folder at `path` into `folder` ("" for the root); its new path.
    public func move(_ path: String, into folder: String) throws -> String {
        guard folder != path, !folder.hasPrefix(path + "/") else { throw FileError.intoItself }
        let name = lastComponent(of: path)
        return try moveItem(path, to: folder.isEmpty ? name : folder + "/" + name)
    }

    /// Moves the file or folder to the macOS Trash, where the Finder can put it back; where it
    /// went.
    @discardableResult
    public func trash(_ path: String) throws -> URL {
        var inTrash: NSURL?
        try FileManager.default.trashItem(at: fileURL(path), resultingItemURL: &inTrash)
        return (inTrash as URL?) ?? fileURL(path)
    }

    /// Renames on disk, never over another file, even one written meanwhile. A change of letter
    /// case only finds the same file at the new name on a case-insensitive volume: it is renamed
    /// all the same.
    private func moveItem(_ path: String, to newPath: String) throws -> String {
        guard newPath != path else { return path }
        let from = fileURL(path).path(percentEncoded: false)
        let to = fileURL(newPath).path(percentEncoded: false)
        let sameFile = newPath.caseInsensitiveCompare(path) == .orderedSame && Self.inode(from) == Self.inode(to)
        if !sameFile, let existing = existingFile(at: newPath) { throw FileError.exists(existing) }
        var renamed = renamex_np(from, to, sameFile ? 0 : UInt32(RENAME_EXCL))
        // A volume that cannot refuse an existing name itself (SMB, exFAT): it was checked above.
        if renamed != 0, errno == ENOTSUP { renamed = renamex_np(from, to, 0) }
        guard renamed == 0 else {
            if errno == EEXIST { throw FileError.exists(existingFile(at: newPath) ?? newPath) }
            throw CocoaError(.fileWriteUnknown, userInfo: [NSUnderlyingErrorKey: POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)])
        }
        return newPath
    }

    private static func inode(_ path: String) -> [UInt64]? {
        var info = stat()
        guard stat(path, &info) == 0 else { return nil }
        return [UInt64(info.st_dev), UInt64(info.st_ino)]
    }

    /// Writes something new in `folder` with `write`, under `name` or, when the name is taken (in
    /// any letter case, on a volume that ignores case as a Mac's does by default), `name 1`,
    /// `name 2`…: never over an existing file, even one another app writes in between. Its path.
    func writeNew(as name: String, extension fileExtension: String?, in folder: String, write: (URL) throws -> Void) throws -> String {
        try FileManager.default.createDirectory(at: fileURL(folder), withIntermediateDirectories: true)
        var number = 0
        while true {
            defer { number += 1 }
            let fileName = (number == 0 ? name : "\(name) \(number)") + (fileExtension.map { "." + $0 } ?? "")
            let path = folder.isEmpty ? fileName : folder + "/" + fileName
            guard existingFile(at: path) == nil else { continue }
            do {
                try write(fileURL(path))
                return path
            } catch CocoaError.fileWriteFileExists {
                continue
            }
        }
    }
}

/// Where `path` is after the file or folder at `old` moved to `new`: nil when the move left it
/// where it was.
public func pathAfterMove(_ path: String, from old: String, to new: String) -> String? {
    if path == old { return new }
    return path.hasPrefix(old + "/") ? new + path.dropFirst(old.count) : nil
}
