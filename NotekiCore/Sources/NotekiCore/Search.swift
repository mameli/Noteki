import Foundation

/// What the user typed in Search: words, all required, and the filters `tag:` and `path:`.
public struct SearchQuery: Equatable, Sendable {
    /// Folded: case and accents do not matter.
    public let words: [String]
    /// Lowercased, without `#`: a Note has the Tag or one under it.
    public let tags: [String]
    /// Folded: the Note's path holds each.
    public let paths: [String]

    public var isEmpty: Bool { words.isEmpty && tags.isEmpty && paths.isEmpty }

    public init(_ text: String) {
        var words: [String] = []
        var tags: [String] = []
        var paths: [String] = []
        for token in Self.tokens(of: text) {
            let lowered = token.lowercased()
            if lowered.hasPrefix("tag:") {
                let tag = lowered.dropFirst(4).drop { $0 == "#" }.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
                if !tag.isEmpty { tags.append(tag) }
            } else if lowered.hasPrefix("path:") {
                let path = token.dropFirst(5)
                if !path.isEmpty { paths.append(fold(String(path))) }
            } else {
                words.append(fold(token))
            }
        }
        self.words = words
        self.tags = tags
        self.paths = paths
    }

    /// The text split at spaces, except between double quotes, which are dropped: `"late order"`
    /// is one word, `path:"Folder 1/"` one filter.
    private static func tokens(of text: String) -> [String] {
        var tokens: [String] = []
        var current = ""
        var quoted = false
        for character in text {
            if character == "\"" {
                quoted.toggle()
            } else if character.isWhitespace, !quoted {
                if !current.isEmpty { tokens.append(current) }
                current = ""
            } else {
                current.append(character)
            }
        }
        if !current.isEmpty { tokens.append(current) }
        return tokens
    }
}

/// What Search found: the first Notes, in order, and how many there are in all.
public struct SearchResults: Equatable, Sendable {
    public let results: [SearchResult]
    public let total: Int

    public static let none = SearchResults(results: [], total: 0)
}

/// A Note that Search found, with the lines that hold a word.
public struct SearchResult: Equatable, Sendable {
    public let note: Note
    /// Every word is in the Note's name: these Notes come first.
    public let nameMatches: Bool
    /// The first lines that hold a word, and how many more do.
    public let lines: [SearchLine]
    public let moreLines: Int
}

/// A line of a Note that holds a word, as Search lists it.
public struct SearchLine: Equatable, Sendable {
    /// From 1, counted as the editor counts lines.
    public let line: Int
    /// The line, cut around the first word found when it is long.
    public let text: String

    public init(line: Int, text: String) {
        self.line = line
        self.text = text
    }
}

extension Vault {
    /// The Notes among `notes` that hold every word of `query`, in their name or text, and pass its
    /// filters: those whose name holds every word first, then by path. Only the first `limit` are
    /// listed, with their lines: past them, a search finds what it was looking for or needs more
    /// words. The files are read now, not kept (ADR 0006): call it off the main thread. Once
    /// `isCancelled`, nothing more is read and nothing is found.
    public func search(
        _ query: SearchQuery, in notes: [Note], limit: Int = 300, isCancelled: @Sendable () -> Bool = { false }
    ) -> SearchResults {
        guard !query.isEmpty else { return .none }
        let candidates = notes.filter { note in
            let path = fold(note.path)
            return query.paths.allSatisfy { path.contains($0) }
        }
        let found: [Found]
        if query.words.isEmpty, query.tags.isEmpty {
            // Only `path:`: the paths are known without reading a file.
            found = candidates.map { Found(note: $0, nameMatches: false) }
        } else {
            found = NoteScan.map(candidates, in: self, isCancelled: isCancelled) { note, text in
                query.matches(note, text: text) ? Found(note: note, nameMatches: query.namesMatch(note)) : nil
            }
        }
        guard !isCancelled() else { return .none }
        let first = found.sorted(by: Found.comesFirst).prefix(limit)
        // The lines of the Notes listed only: read again, a few hundred files at most.
        let lines = Dictionary(uniqueKeysWithValues: NoteScan.map(first.map(\.note), in: self, isCancelled: isCancelled) { note, text in
            (note, query.lines(in: text))
        })
        guard !isCancelled() else { return .none }
        let results = first.map { found in
            let listed = lines[found.note] ?? Lines(listed: [], more: 0)
            return SearchResult(note: found.note, nameMatches: found.nameMatches, lines: listed.listed, moreLines: listed.more)
        }
        return SearchResults(results: results, total: found.count)
    }
}

/// A Note that holds the words, before its lines are read.
private struct Found: Sendable {
    let note: Note
    let nameMatches: Bool

    /// Those whose name holds every word first, then by path.
    static func comesFirst(_ a: Found, _ b: Found) -> Bool {
        (a.nameMatches ? 0 : 1, a.note.path) < (b.nameMatches ? 0 : 1, b.note.path)
    }
}

/// The lines listed for a Note, and how many more hold a word.
private struct Lines: Sendable {
    let listed: [SearchLine]
    let more: Int
}

extension SearchQuery {
    /// The lines listed for one Note: enough to see where it speaks of the words.
    private static let linesListed = 10

    fileprivate func matches(_ note: Note, text: String) -> Bool {
        let name = fold(note.name)
        let folded = fold(text)
        guard words.allSatisfy({ name.contains($0) || folded.contains($0) }) else { return false }
        guard !tags.isEmpty else { return true }
        let noteTags = TagMap.tags(in: text).map { $0.lowercased() }
        return tags.allSatisfy { tag in noteTags.contains { $0 == tag || $0.hasPrefix(tag + "/") } }
    }

    fileprivate func namesMatch(_ note: Note) -> Bool {
        let name = fold(note.name)
        return !words.isEmpty && words.allSatisfy(name.contains)
    }

    /// The first lines holding a word, cut around it, and how many more there are.
    fileprivate func lines(in text: String) -> Lines {
        guard !words.isEmpty else { return Lines(listed: [], more: 0) }
        let original = MarkdownText.lines(of: text)
        // Folding changes no line break, so the lines are the same lines; it may change their length.
        let folded = MarkdownText.lines(of: fold(text))
        var lines: [SearchLine] = []
        var more = 0
        for (index, line) in folded.enumerated() where index < original.count {
            guard let word = words.first(where: line.contains) else { continue }
            guard lines.count < Self.linesListed else {
                more += 1
                continue
            }
            // Where the word is in the line as written: folding may change its length (ß is ss).
            let written = original[index]
            let range = written.range(of: word, options: [.caseInsensitive, .diacriticInsensitive])
            let offset = range.map { written.distance(from: written.startIndex, to: $0.lowerBound) } ?? 0
            lines.append(SearchLine(line: index + 1, text: MarkdownText.excerpt(of: written, around: offset)))
        }
        return Lines(listed: lines, more: more)
    }
}
