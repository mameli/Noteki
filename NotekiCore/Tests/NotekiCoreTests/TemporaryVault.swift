import Foundation

/// A Vault in a temporary folder, deleted when the test ends.
final class TemporaryVault {
    let url: URL

    init(files: [String: String]) throws {
        url = FileManager.default.temporaryDirectory
            .appending(path: "NotekiTests-\(UUID().uuidString)", directoryHint: .isDirectory)
        for (path, content) in files {
            try write(content, at: path)
        }
    }

    deinit {
        try? FileManager.default.removeItem(at: url)
    }

    func write(_ content: String, at path: String) throws {
        let file = url.appending(path: path)
        try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data(content.utf8).write(to: file)
    }

    func read(_ path: String) throws -> String {
        try String(contentsOf: url.appending(path: path), encoding: .utf8)
    }

    func data(_ path: String) throws -> Data {
        try Data(contentsOf: url.appending(path: path))
    }

    func move(_ path: String, to newPath: String) throws {
        let destination = url.appending(path: newPath)
        try FileManager.default.createDirectory(at: destination.deletingLastPathComponent(), withIntermediateDirectories: true)
        try FileManager.default.moveItem(at: url.appending(path: path), to: destination)
    }

    func delete(_ path: String) throws {
        try FileManager.default.removeItem(at: url.appending(path: path))
    }

    /// As most editors save: a new file put in place of the old one.
    func replace(_ path: String, with content: String) throws {
        try Data(content.utf8).write(to: url.appending(path: path), options: .atomic)
    }

    /// Every file under the Vault with its content, to check nothing changed.
    func snapshot() throws -> [String: Data] {
        var files: [String: Data] = [:]
        let enumerator = FileManager.default.enumerator(at: url, includingPropertiesForKeys: [.isRegularFileKey])
        while let file = enumerator?.nextObject() as? URL {
            guard try file.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile == true else { continue }
            files[file.path(percentEncoded: false)] = try Data(contentsOf: file)
        }
        return files
    }
}
