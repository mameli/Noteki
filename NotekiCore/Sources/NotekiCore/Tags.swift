import Foundation

/// A Tag in the tree of Tags (CONTEXT.md): `client/axo` is `axo` under `client`.
public struct TagNode: Equatable, Sendable, Identifiable {
    /// This level's name, as most Notes write it: `axo`.
    public let name: String
    /// The Tag from the first level, lowercased, as `TagMap.notes(tagged:)` takes it: `client/axo`.
    public let path: String
    /// The Notes with this Tag or one under it, each counted once.
    public let noteCount: Int
    public let children: [TagNode]

    public var id: String { path }
}

/// The Tags of each Note, kept in memory beside the index of files (ADR 0006): the spellings
/// once, and for each Note the numbers of those it uses. A Tag is in the `tags` Property (a list
/// or one value, with or without `#`) or written `#tag` in the text outside code. Tags are the
/// same in any letter case.
public struct TagMap: NoteScanned {
    private var spellings: [String] = []
    private var spellingIDs: [String: Int32] = [:]
    /// The Tags of each Note, each spelling once. A Note without Tags is not here.
    private var tags: [Note: [Int32]] = [:]

    public init() {}

    public mutating func rescan(_ notes: [Note], in vault: Vault) {
        NoteScan.read(notes, in: vault, into: &self, parse: Self.tags(in:), merge: { map, note, tags in
            map.set(note, spellings: tags ?? [])
        })
    }

    public mutating func remove(_ note: Note) {
        tags[note] = nil
    }

    private mutating func set(_ note: Note, spellings found: [String]) {
        guard !found.isEmpty else { return remove(note) }
        tags[note] = found.map { spelling in
            if let id = spellingIDs[spelling] { return id }
            let id = Int32(spellings.count)
            spellings.append(spelling)
            spellingIDs[spelling] = id
            return id
        }
    }

    /// The Tags as a tree, each level by name, with the number of Notes at or under it.
    public func tree() -> [TagNode] {
        var counts: [String: Int] = [:]
        // The spellings of each level, by Notes that write them: the most written is shown.
        var levels: [String: [String: Int]] = [:]
        for ids in tags.values {
            var seen = Set<String>()
            for id in ids {
                let components = spellings[Int(id)].split(separator: "/").map(String.init)
                var path = ""
                for component in components {
                    path += (path.isEmpty ? "" : "/") + component.lowercased()
                    levels[path, default: [:]][component, default: 0] += 1
                    if seen.insert(path).inserted { counts[path, default: 0] += 1 }
                }
            }
        }
        // The most written wins; between equals, the lowercase one, then the first alphabetically.
        func rank(_ spelling: (key: String, value: Int)) -> (Int, Int, String) {
            (-spelling.value, spelling.key == spelling.key.lowercased() ? 0 : 1, spelling.key)
        }
        func name(of path: String) -> String {
            levels[path, default: [:]].min { rank($0) < rank($1) }?.key ?? path
        }
        // Each level under its parent once, so that the tree is made in one pass.
        var below: [String: [String]] = [:]
        for path in counts.keys {
            below[path.lastIndex(of: "/").map { String(path[..<$0]) } ?? "", default: []].append(path)
        }
        func nodes(under parent: String) -> [TagNode] {
            below[parent, default: []].sorted().map { path in
                TagNode(name: name(of: path), path: path, noteCount: counts[path, default: 0], children: nodes(under: path))
            }
        }
        return nodes(under: "")
    }

    /// The Notes with the Tag or a Tag under it, by path. `tag` is a path of the tree, in any case.
    public func notes(tagged tag: String) -> [Note] {
        let wanted = tag.lowercased()
        // Which spellings are the Tag or under it, once, rather than for every Note.
        let matching = Set(spellings.indices.filter { index in
            let found = spellings[index].lowercased()
            return found == wanted || found.hasPrefix(wanted + "/")
        }.map(Int32.init))
        return tags.filter { _, ids in ids.contains { matching.contains($0) } }.keys.sorted { $0.path < $1.path }
    }

    // MARK: Reading the Tags of a text

    /// The Tags of a Markdown text: the `tags` Property, then `#tag` in the text. Each spelling once.
    static func tags(in text: String) -> [String] {
        var found: [String] = []
        MarkdownText.forEachLine(of: text, skippingFrontmatter: true, frontmatter: { found += frontmatterTags(in: $0) }) { _, line in
            if line.contains(hash) { found += hashtags(in: line) }
        }
        var seen = Set<String>()
        return found.filter { seen.insert($0).inserted }
    }

    private static let hash: UInt8 = 35

    /// The `#tag`s of a line, outside code spans: after the start of the line or a space, a letter
    /// first, then letters, digits, `_`, `-` and `/`.
    private static func hashtags(in line: ArraySlice<UInt8>) -> [String] {
        let bytes = Array(line)
        var tags: [String] = []
        var i = 0
        while i < bytes.count {
            switch bytes[i] {
            case MarkdownText.backslash:
                // An escaped character is text: `\#tag` is no Tag.
                i += 2
            case MarkdownText.backtick:
                let run = bytes[i...].prefix { $0 == MarkdownText.backtick }.count
                i = MarkdownText.endOfCodeSpan(in: bytes, after: i + run, length: run) ?? i + run
            case hash where i == 0 || bytes[i - 1] == MarkdownText.space || bytes[i - 1] == MarkdownText.tab:
                // To the next ASCII character a Tag cannot hold; the others are told apart below.
                var end = i + 1
                while end < bytes.count, bytes[end] >= 0x80 || isTagByte(bytes[end]) { end += 1 }
                if let tag = validTag(String(decoding: bytes[(i + 1)..<end], as: UTF8.self)) { tags.append(tag) }
                i = max(end, i + 1)
            default:
                i += 1
            }
        }
        return tags
    }

    private static func isTagByte(_ byte: UInt8) -> Bool {
        (byte >= 48 && byte <= 57) || (byte >= 65 && byte <= 90) || (byte >= 97 && byte <= 122)
            || byte == 95 || byte == 45 || byte == 47
    }

    /// The Tag a text starts with, nil if it starts with no letter: its characters are letters,
    /// digits, `_`, `-` and `/`, without an empty level.
    private static func validTag(_ text: String) -> String? {
        let tag = String(text.prefix { $0.isLetter || $0.isDecimalDigit || $0 == "_" || $0 == "-" || $0 == "/" })
        guard tag.first?.isLetter == true else { return nil }
        // A level left empty ends the Tag: `a//b` is `a`, `a/` is `a`.
        let levels = tag.split(separator: "/", omittingEmptySubsequences: false)
        return String(levels.prefix { !$0.isEmpty }.joined(separator: "/"))
    }

    /// The value without a YAML comment: from a `#` after a space and followed by a space, as
    /// `alpha # old`. `#beta` after a space is a Tag written with its `#`.
    private static func withoutComment(_ value: String) -> String {
        guard let comment = value.firstRange(of: /\s#(\s|$)/) else { return value }
        return value[..<comment.lowerBound].trimmingCharacters(in: .whitespaces)
    }

    /// The Tags in the `tags` Property of the Frontmatter's lines: a block list, a flow list on
    /// one line or several, or one value, or values separated by commas or spaces.
    private static func frontmatterTags(in lines: [String]) -> [String] {
        var values: [Substring] = []
        var index = 0
        while index < lines.count {
            defer { index += 1 }
            let line = lines[index]
            guard line.hasPrefix("tags"), case let rest = line.dropFirst(4).drop(while: { $0 == " " }),
                  rest.first == ":" else { continue }
            let value = rest.dropFirst().trimmingCharacters(in: .whitespaces)
            if value.isEmpty || value.hasPrefix("#") {
                // A block list: the lines that follow start with a dash, blank lines between them.
                while index + 1 < lines.count {
                    let item = lines[index + 1].drop { $0 == " " }
                    if item.isEmpty {
                        index += 1
                        continue
                    }
                    guard item.first == "-" else { break }
                    values.append(item.dropFirst())
                    index += 1
                }
            } else if value.hasPrefix("[") {
                var list = value.dropFirst()
                while !list.contains("]"), index + 1 < lines.count {
                    index += 1
                    list += " " + lines[index]
                }
                values += list.prefix { $0 != "]" }.split(separator: ",")
            } else {
                values += withoutComment(String(value)).split { $0 == "," || $0 == " " || $0 == "\t" }
            }
        }
        return values.compactMap { value in
            var item = withoutComment(value.trimmingCharacters(in: .whitespaces))
            if let quote = item.first, quote == "\"" || quote == "'" { item = item.trimmingCharacters(in: CharacterSet(charactersIn: String(quote))) }
            if item.hasPrefix("#") { item.removeFirst() }
            // YAML's empty values are no Tag.
            return validTag(item) == item && item != "null" ? item : nil
        }
    }
}

private extension Character {
    /// 0–9 and the digits of other scripts, not superscripts or fractions.
    var isDecimalDigit: Bool {
        unicodeScalars.allSatisfy { $0.properties.numericType == .decimal }
    }
}
