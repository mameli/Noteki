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

/// The Vault's files by name, kept in memory to resolve and complete Wikilinks and for the
/// Quick switcher (ADR 0006). Built from the listing, so it costs no reading of the Notes.
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
    /// The Notes, in the order Files lists them.
    public let notes: [Note]
    /// Their names as the Quick switcher matches them, in the same order.
    let foldedNames: [String]

    public init(tree: [VaultItem]) {
        var byName: [String: [Candidate]] = [:]
        var notes: [Note] = []
        func index(_ item: VaultItem) {
            let path = item.path.lowercased()
            byName[lastComponent(of: path), default: []].append(Candidate(item: item, path: path))
        }
        func add(_ items: [VaultItem]) {
            for item in items {
                switch item {
                case .folder(_, let children): add(children)
                case .note(let note):
                    notes.append(note)
                    index(item)
                case .attachment: index(item)
                }
            }
        }
        add(tree)
        self.byName = byName
        self.notes = notes
        foldedNames = notes.map { fold($0.name) }
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

    /// The Notes offered when the user types `[[`: those in `recent` first, in that order (the
    /// latest first), then the others as Files lists them.
    public func linkCompletions(recent: [Note]) -> LinkCompletions {
        let listed = Set(notes)
        var offered = Set<Note>()
        let recent = recent.filter { listed.contains($0) && offered.insert($0).inserted }
        return LinkCompletions(
            recent: recent.map(completion(for:)),
            others: notes.filter { !offered.contains($0) }.map(completion(for:))
        )
    }

    private func completion(for note: Note) -> LinkCompletion {
        let components = note.path.split(separator: "/", omittingEmptySubsequences: false)
        return LinkCompletion(
            name: note.name,
            folder: note.folder,
            link: shortestLink(to: note, components: components)
        )
    }

    /// The name, or else the fewest folders before it that only this Note's path ends with, so
    /// that the link leads to it from any Note, as `resolve` (and Obsidian) reads it. Obsidian
    /// itself writes the whole path as soon as the name is shared; this is shorter and leads to
    /// the same Note.
    private func shortestLink(to note: Note, components: [Substring]) -> String {
        let name = lastComponent(of: note.path).lowercased()
        let candidates = byName[name] ?? []
        for count in 1...components.count {
            let suffix = components.suffix(count).joined(separator: "/")
            // As `resolve` compares: the text's end, not whole folders, so `Data/` ends in `a/`.
            let lowered = suffix.lowercased()
            let link = (suffix as NSString).deletingPathExtension
            // Unique among the Notes (the whole path is, as an exact match), and not taken by an
            // Attachment: `Report.pdf` is the PDF's name before it is the Note `Report.pdf.md`'s.
            let unique = count == components.count || candidates.count(where: { $0.path.hasSuffix(lowered) }) == 1
            if unique, resolve(link, from: nil) == .note(note) {
                return link
            }
        }
        // With its extension the whole path is an exact match: nothing else can take it.
        return note.path
    }

    private func shortestFirst(_ candidates: [Candidate]) -> [Candidate] {
        candidates.enumerated()
            .sorted { ($0.element.path.count, $0.offset) < ($1.element.path.count, $1.offset) }
            .map(\.element)
    }
}

/// What `[[` offers: the Notes opened recently, the latest first, then all the others.
public struct LinkCompletions: Equatable, Sendable, Encodable {
    public let recent: [LinkCompletion]
    public let others: [LinkCompletion]

    /// Nothing to offer: no Vault listed yet, or a Loose file.
    public static let none = LinkCompletions(recent: [], others: [])

    public init(recent: [LinkCompletion], others: [LinkCompletion]) {
        self.recent = recent
        self.others = others
    }
}

/// A Note offered to complete a Wikilink.
public struct LinkCompletion: Equatable, Sendable, Encodable {
    /// What the user's typing is matched against.
    public let name: String
    /// Where the Note is, shown beside its name; empty at the Vault's root.
    public let folder: String
    /// What is written between the brackets when the Note is chosen.
    public let link: String

    public init(name: String, folder: String, link: String) {
        self.name = name
        self.folder = folder
        self.link = link
    }
}

/// What follows the last `/`, empty after a final one, as Obsidian takes a link's name.
private func afterLastSlash(_ path: String) -> String {
    guard let slash = path.lastIndex(of: "/") else { return path }
    return String(path[path.index(after: slash)...])
}

/// Case and accents do not matter: `Café` is found by `cafe`.
func fold(_ text: String) -> String {
    text.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil)
}

func folderOf(_ path: String) -> String {
    guard let slash = path.lastIndex(of: "/") else { return "" }
    return String(path[..<slash])
}
