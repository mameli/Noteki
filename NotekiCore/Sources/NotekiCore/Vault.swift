import Foundation

/// The folder of Notes and Attachments Noteki has open. It only reads: nothing is ever written
/// inside it but the Notes the user edits (ADR 0002).
public struct Vault: Sendable {
    public let root: URL

    public init(root: URL) {
        self.root = root.standardizedFileURL
    }

    /// The folders, Notes and Attachments of the Vault: at each level folders first, then the
    /// files, each group in Finder order. Other files are not listed.
    public func tree() throws -> [VaultItem] {
        try items(in: root, relativePath: "")
    }

    /// The text of a Note as it is on disk.
    public func text(of note: Note) throws -> String {
        try String(contentsOf: url(of: note), encoding: .utf8)
    }

    func url(of note: Note) -> URL {
        fileURL(note.path)
    }

    /// Where the Attachment is on disk, for the views that show it.
    public func url(of attachment: Attachment) -> URL {
        fileURL(attachment.path)
    }

    /// The Vault's change entry: the files the file system reports as changed, as paths in the
    /// Vault. Those outside it or in an Excluded folder (a `.git` written by obsidian-git, say)
    /// are left out. The Vault does not watch: the app's watcher calls this (ticket 06).
    public func changedPaths(_ urls: [URL]) -> Set<String> {
        let rootPath = Self.prefix(Self.realPath(root))
        return Set(urls.compactMap { path(ofChangeAt: Self.realPath($0), under: rootPath) })
    }

    /// The root as a prefix of the paths under it: `/Users/me/Vault/`, or `/` for the disk's root.
    private static func prefix(_ rootPath: String) -> String {
        rootPath.hasSuffix("/") ? rootPath : rootPath + "/"
    }

    /// Where a file or folder opened from the Finder or another app (Takku) is.
    public enum Place: Equatable, Sendable {
        case note(Note)
        case attachment(Attachment)
        /// A folder of the Vault, by its path; "" for the Vault itself.
        case folder(String)
        /// Inside the Vault but not listed: in an Excluded folder, or neither a Note nor an
        /// Attachment. Not to be opened on its own: it is the Vault's.
        case excluded
        case outside
    }

    public func place(_ url: URL) -> Place {
        let rootPath = Self.realPath(root)
        let candidate = Self.realPath(url)
        if candidate == rootPath { return .folder("") }
        let prefix = Self.prefix(rootPath)
        guard candidate.hasPrefix(prefix) else { return .outside }
        guard let path = self.path(ofChangeAt: candidate, under: prefix) else { return .excluded }
        var isFolder: ObjCBool = false
        guard FileManager.default.fileExists(atPath: candidate, isDirectory: &isFolder) else { return .outside }
        if isFolder.boolValue { return .folder(path) }
        if (path as NSString).pathExtension == Note.fileExtension { return .note(Note(path: path)) }
        return Attachment(path: path).map(Place.attachment) ?? .excluded
    }

    /// `candidate` is a real path (`realPath`).
    private func path(ofChangeAt candidate: String, under rootPath: String) -> String? {
        guard candidate.hasPrefix(rootPath) else { return nil }
        let path = String(candidate.dropFirst(rootPath.count))
        return isInExcludedFolder(path) ? nil : path
    }

    /// The path with every symbolic link resolved (/var → /private/var), also for a file that is
    /// gone: Foundation's own resolution keeps /private only for files that exist, so the event of
    /// a renamed or deleted file would not match the Vault's root.
    private static func realPath(_ url: URL) -> String {
        var existing = url.standardizedFileURL
        var rest: [String] = []
        while !FileManager.default.fileExists(atPath: existing.path(percentEncoded: false)), existing.pathComponents.count > 1 {
            rest.insert(existing.lastPathComponent, at: 0)
            existing.deleteLastPathComponent()
        }
        guard let resolved = realpath(existing.path(percentEncoded: false), nil) else {
            return url.standardizedFileURL.path(percentEncoded: false)
        }
        defer { free(resolved) }
        return ([String(cString: resolved)] + rest).joined(separator: "/")
    }

    /// Whether any folder on the way to this path, or the path itself, is an Excluded folder.
    private func isInExcludedFolder(_ path: String) -> Bool {
        var folder = root
        for component in path.split(separator: "/") {
            folder = folder.appending(path: String(component))
            if isExcludedFolder(folder) { return true }
        }
        return false
    }

    private func fileURL(_ path: String) -> URL {
        root.appending(path: path, directoryHint: .notDirectory)
    }

    private func items(in folder: URL, relativePath: String) throws -> [VaultItem] {
        let contents = try FileManager.default.contentsOfDirectory(
            at: folder, includingPropertiesForKeys: [.isDirectoryKey], options: []
        )
        var folders: [VaultItem] = []
        var files: [VaultItem] = []
        for url in contents {
            let name = url.lastPathComponent
            let path = relativePath.isEmpty ? name : relativePath + "/" + name
            // A file gone while listing (an editor's temporary file) is simply not listed.
            guard let isDirectory = try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory else { continue }
            if isDirectory {
                // A folder the user cannot read is left out rather than hiding the whole Vault.
                guard !isExcludedFolder(url), let children = try? items(in: url, relativePath: path) else {
                    continue
                }
                folders.append(.folder(path: path, children: children))
            } else if url.pathExtension == Note.fileExtension {
                files.append(.note(Note(path: path)))
            } else if let attachment = Attachment(path: path) {
                files.append(.attachment(attachment))
            }
        }
        return folders.sorted(by: finderOrder) + files.sorted(by: finderOrder)
    }

    /// Environments and dependencies of code kept inside the Vault: thousands of files, no Notes.
    private static let codeFolderNames: Set<String> = ["node_modules", "venv", "__pycache__"]

    /// Excluded folders are neither listed nor read: hidden ones (`.obsidian`, `.venv`), the
    /// environments and dependencies of code, and any folder that is a repository of its own,
    /// whose `.git` is a folder or, for submodules and worktrees, a file. Only asked of folders
    /// below the root, so a Vault kept in git (obsidian-git) is not excluded from itself.
    private func isExcludedFolder(_ folder: URL) -> Bool {
        let name = folder.lastPathComponent
        return name.hasPrefix(".")
            || Self.codeFolderNames.contains(name)
            || FileManager.default.fileExists(atPath: folder.appending(path: ".git").path(percentEncoded: false))
    }
}

/// One entry of the Vault's tree.
public enum VaultItem: Hashable, Sendable {
    /// A folder, by its path from the Vault's root, with what it holds.
    case folder(path: String, children: [VaultItem])
    case note(Note)
    case attachment(Attachment)

    /// From the Vault's root: `Meetings/Transcripts` or `Meetings/Retro.md`.
    public var path: String {
        switch self {
        case .folder(let path, _): path
        case .note(let note): note.path
        case .attachment(let attachment): attachment.path
        }
    }

    /// The folder's name, the Note's name without `.md`, or the Attachment's file name.
    public var name: String {
        switch self {
        case .folder(let path, _): lastComponent(of: path)
        case .note(let note): note.name
        case .attachment(let attachment): attachment.name
        }
    }
}

/// A Markdown file in the Vault, identified by its path from the Vault's root.
public struct Note: Hashable, Sendable {
    public static let fileExtension = "md"

    /// From the Vault's root, with `/` between folders: `Meetings/Retro.md`.
    public let path: String

    public init(path: String) {
        self.path = path
    }

    /// The file name without `.md`, the name Wikilinks use (or another extension, for a text
    /// file opened on its own).
    public var name: String {
        (lastComponent(of: path) as NSString).deletingPathExtension
    }
}

/// An image or a PDF in the Vault: Noteki shows it but does not edit it.
public struct Attachment: Hashable, Sendable {
    public enum Kind: Sendable {
        case image, pdf
    }

    private static let kinds: [String: Kind] = [
        "png": .image, "jpg": .image, "jpeg": .image, "gif": .image, "webp": .image, "svg": .image, "heic": .image,
        "pdf": .pdf,
    ]

    /// From the Vault's root, with `/` between folders: `Meetings/Pasted image 20260917102056.png`.
    public let path: String
    public let kind: Kind

    /// Nil for a file that is not an image or a PDF. Extensions in any case: phones write `.JPG`.
    public init?(path: String) {
        let fileExtension = (lastComponent(of: path) as NSString).pathExtension.lowercased()
        guard let kind = Self.kinds[fileExtension] else { return nil }
        self.path = path
        self.kind = kind
    }

    /// The file name with its extension, as Embeds write it: `![[Pasted image 20260917102056.png]]`.
    public var name: String { lastComponent(of: path) }
}

private func finderOrder(_ a: VaultItem, _ b: VaultItem) -> Bool {
    a.name.localizedStandardCompare(b.name) == .orderedAscending
}

private func lastComponent(of path: String) -> String {
    path.split(separator: "/").last.map(String.init) ?? path
}
