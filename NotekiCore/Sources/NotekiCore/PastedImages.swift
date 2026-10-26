import Foundation

extension Vault {
    /// Saves a Pasted image, PNG data, in the Note's folder as `Pasted image YYYYMMDDHHmmss.png`
    /// (Obsidian's name, the moment in the Mac's time zone), and gives its Embed to write at the
    /// cursor. A name already taken gets a number, as `Pasted image 20261026093005 1.png`.
    public func pasteImage(_ png: Data, into note: Note, at date: Date = .now) throws -> String {
        let name = "Pasted image " + Self.pastedImageMoment.string(from: date)
        let path = try writeNew(as: name, extension: "png", in: note.folder) { file in
            try png.write(to: file, options: .withoutOverwriting)
        }
        return Self.embed(lastComponent(of: path))
    }

    /// The Embeds of files dropped from the Finder on a Note, one per line; nil when none is an
    /// image. An image from outside the Vault is copied next to the Note, keeping its name, with
    /// a number if it is taken; one the Vault lists is embedded where it is, by its name when
    /// that leads to it from the Note, otherwise by its path. A file that cannot be copied is left
    /// out and the others embedded, so no copy is left in the Vault without its Embed; throws only
    /// when none of the images could be.
    public func dropFiles(_ files: [URL], onto note: Note, in index: VaultIndex) throws -> String? {
        var embeds: [String] = []
        var failure: (any Error)?
        for file in files {
            guard Attachment(path: file.lastPathComponent)?.kind == .image,
                  (try? file.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == false else { continue }
            if case .attachment(let listed) = place(file) {
                let byName = index.resolve(listed.name, from: note) == .attachment(listed)
                embeds.append(Self.embed(byName ? listed.name : listed.path))
                continue
            }
            let name = Self.linkable((file.lastPathComponent as NSString).deletingPathExtension)
            do {
                let copied = try writeNew(as: name, extension: file.pathExtension, in: note.folder) { destination in
                    try FileManager.default.copyItem(at: file, to: destination)
                }
                embeds.append(Self.embed(lastComponent(of: copied)))
            } catch {
                failure = failure ?? error
            }
        }
        if embeds.isEmpty, let failure { throw failure }
        return embeds.isEmpty ? nil : embeds.joined(separator: "\n")
    }

    private static func embed(_ link: String) -> String {
        "![[\(link)]]"
    }

    /// The name without what a Wikilink cannot hold (`[`, `]`, `|`, `#`, `^`) nor what Obsidian
    /// refuses on a Mac (`\`, `:`), which would make an Embed that leads nowhere: those become
    /// single spaces, trimmed. Nothing left, `image`.
    private static func linkable(_ name: String) -> String {
        let cleaned = name.split(whereSeparator: { "[]|#^\\: ".contains($0) }).joined(separator: " ")
        return cleaned.isEmpty ? "image" : cleaned
    }

    private static let pastedImageMoment: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.dateFormat = "yyyyMMddHHmmss"
        return formatter
    }()
}
