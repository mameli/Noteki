import Foundation

/// The folder of Notes and Attachments Noteki has open. It only reads: nothing is ever written
/// inside it but the Notes the user edits (ADR 0002).
public struct Vault: Sendable {
    public let root: URL

    public init(root: URL) {
        self.root = root.standardizedFileURL
    }

    /// The folders and Notes of the Vault, folders first, each level in Finder order.
    public func tree() throws -> [VaultItem] {
        try items(in: root, relativePath: "")
    }

    /// The text of a Note as it is on disk.
    public func text(of note: Note) throws -> String {
        try String(contentsOf: url(of: note), encoding: .utf8)
    }

    func url(of note: Note) -> URL {
        root.appending(path: note.path, directoryHint: .notDirectory)
    }

    private func items(in folder: URL, relativePath: String) throws -> [VaultItem] {
        let contents = try FileManager.default.contentsOfDirectory(
            at: folder, includingPropertiesForKeys: [.isDirectoryKey], options: []
        )
        var folders: [VaultItem] = []
        var notes: [VaultItem] = []
        for url in contents {
            let name = url.lastPathComponent
            let path = relativePath.isEmpty ? name : relativePath + "/" + name
            if try url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory == true {
                folders.append(.folder(path: path, children: try items(in: url, relativePath: path)))
            } else if url.pathExtension == Note.fileExtension {
                notes.append(.note(Note(path: path)))
            }
        }
        return folders.sorted(by: finderOrder) + notes.sorted(by: finderOrder)
    }
}

/// One entry of the Vault's tree.
public enum VaultItem: Hashable, Sendable {
    /// A folder, by its path from the Vault's root, with what it holds.
    case folder(path: String, children: [VaultItem])
    case note(Note)

    /// From the Vault's root: `Meetings/Transcripts` or `Meetings/Retro.md`.
    public var path: String {
        switch self {
        case .folder(let path, _): path
        case .note(let note): note.path
        }
    }

    /// The folder's name, or the Note's name without `.md`.
    public var name: String {
        switch self {
        case .folder(let path, _): lastComponent(of: path)
        case .note(let note): note.name
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

    /// The file name without `.md`, the name Wikilinks use.
    public var name: String {
        String(lastComponent(of: path).dropLast(Note.fileExtension.count + 1))
    }
}

private func finderOrder(_ a: VaultItem, _ b: VaultItem) -> Bool {
    a.name.localizedStandardCompare(b.name) == .orderedAscending
}

private func lastComponent(of path: String) -> String {
    path.split(separator: "/").last.map(String.init) ?? path
}
