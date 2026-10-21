import Foundation

/// A link to a Note written `[[name]]`: what is between the brackets, split into the Note it
/// points to and the Alias it shows instead.
public struct Wikilink: Equatable, Sendable {
    /// The name or path of the Note, without the heading or block it may point to.
    public let target: String
    /// `[[name|alias]]`: the text shown instead of the name.
    public let alias: String?

    /// `inner` is the text between `[[` and `]]`.
    public init(inner: String) {
        // In a table Obsidian writes the pipe escaped: `[[name\|alias]]`.
        let unescaped = inner.replacingOccurrences(of: "\\|", with: "|")
        let parts = unescaped.split(separator: "|", maxSplits: 1, omittingEmptySubsequences: false)
        let destination = parts[0].split(separator: "#", maxSplits: 1, omittingEmptySubsequences: false)[0]
        target = destination.trimmingCharacters(in: .whitespaces)
        alias = parts.count > 1 ? parts[1].trimmingCharacters(in: .whitespaces) : nil
    }
}

/// The Vault's files by name, kept in memory to resolve Wikilinks (ADR 0006). Built from the
/// listing, so it costs no reading of the Notes.
public struct VaultIndex: Sendable {
    /// A file and its lowercased path, compared many times while resolving.
    private struct Candidate: Sendable {
        let item: VaultItem
        let path: String
    }

    /// Lowercased file name, with its extension → the Notes and Attachments with that name, in
    /// the order Files lists them. Only listed files: not those in Excluded folders, nor files
    /// Noteki does not show (Obsidian's `.canvas`, say).
    private let byName: [String: [Candidate]]

    public init(tree: [VaultItem]) {
        var byName: [String: [Candidate]] = [:]
        func add(_ items: [VaultItem]) {
            for item in items {
                switch item {
                case .folder(_, let children): add(children)
                case .note, .attachment:
                    let path = item.path.lowercased()
                    byName[lastComponent(of: path), default: []].append(Candidate(item: item, path: path))
                }
            }
        }
        add(tree)
        self.byName = byName
    }

    /// The Note or Attachment a Wikilink's target leads to from `source`, exactly as Obsidian
    /// resolves it (ADR 0002): by file name in any case, `.md` optional; a path from the root or
    /// relative to the source's folder; among several with the name, those whose path starts
    /// with the source's folder first, then the shortest path. Nil for an Unresolved link.
    public func resolve(_ target: String, from source: Note?) -> VaultItem? {
        // `[[#heading]]`, `[[|alias]]`: the Note that holds the link.
        if target.isEmpty { return source.map(VaultItem.note) }
        var link = target.lowercased()
        var name = afterLastSlash(link)
        var candidates = name.contains(".") ? byName[name] : nil
        if candidates == nil {
            link = (target + "." + Note.fileExtension).lowercased()
            name = afterLastSlash(link)
            candidates = byName[name]
        }
        guard let candidates, !candidates.isEmpty else { return nil }
        if name == link, candidates.count == 1 { return candidates[0].item }

        var folder = folderOf(source?.path ?? "").lowercased()
        if link.hasPrefix("./") || link.hasPrefix("../") {
            if link.hasPrefix("./../") { link.removeFirst(2) }
            if link.hasPrefix("./") {
                if !folder.isEmpty { folder += "/" }
                link = folder + link.dropFirst(2)
            } else {
                while link.hasPrefix("../") {
                    link.removeFirst(3)
                    folder = folderOf(folder)
                }
                if !folder.isEmpty { folder += "/" }
                link = folder + link
            }
            if let exact = candidates.first(where: { $0.path == link }) { return exact.item }
        }
        if link.hasPrefix("/") { link.removeFirst() }
        if let exact = candidates.first(where: { $0.path == link }) { return exact.item }
        if target.hasPrefix("/") { return nil }
        // As Obsidian compares them: text prefixes and suffixes, not folder boundaries; ties in
        // length keep the listing's order, as Obsidian keeps its own.
        let matching = candidates.filter { $0.path.hasSuffix(link) }
        let near = matching.filter { $0.path.hasPrefix(folder) }
        let far = matching.filter { !$0.path.hasPrefix(folder) }
        return (shortestFirst(near) + shortestFirst(far)).first?.item
    }

    private func shortestFirst(_ candidates: [Candidate]) -> [Candidate] {
        candidates.enumerated()
            .sorted { ($0.element.path.count, $0.offset) < ($1.element.path.count, $1.offset) }
            .map(\.element)
    }
}

/// What follows the last `/`, empty after a final one, as Obsidian takes a link's name.
private func afterLastSlash(_ path: String) -> String {
    guard let slash = path.lastIndex(of: "/") else { return path }
    return String(path[path.index(after: slash)...])
}

private func folderOf(_ path: String) -> String {
    guard let slash = path.lastIndex(of: "/") else { return "" }
    return String(path[..<slash])
}
