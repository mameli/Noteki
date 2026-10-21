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
