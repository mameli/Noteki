import Foundation

/// A Wikilink in another Note that points to a Note (CONTEXT.md): the Note that holds it, the
/// line it is on and that line as it reads.
public struct Backlink: Equatable, Sendable {
    public let note: Note
    /// From 1, counted as the editor counts lines.
    public let line: Int
    public let text: String

    public init(note: Note, line: Int, text: String) {
        self.note = note
        self.line = line
        self.text = text
    }
}

/// Where the Wikilinks and Embeds are written, kept in memory beside the index of files
/// (ADR 0006). For each Note it keeps the file names its links ask for and their lines, not the
/// links themselves: 8 bytes a link, as every link of the Vault is here. What a link says, and
/// which Note it leads to, is read from the file when a Backlink is asked for, with the index as
/// it is then, so a Note created or renamed later is found.
public struct LinkMap: NoteScanned {
    /// A link's place: the file name it asks for (an id in `nameIDs`) and its line.
    private struct Place: Sendable {
        let nameID: Int32
        let line: Int32
    }

    /// The names links ask for, lowercased and without `.md`: only a link asking for a Note's
    /// name can lead to that Note.
    private var nameIDs: [String: Int32] = [:]
    private var places: [Note: [Place]] = [:]

    public init() {}

    /// Reads `notes` again: their links replace the ones kept, a Note that cannot be read (deleted,
    /// not UTF-8) has none.
    public mutating func rescan(_ notes: [Note], in vault: Vault) {
        NoteScan.read(notes, in: vault, into: &self, parse: { text in
            var links: [(name: String, line: Int)] = []
            MarkdownText.forEachLine(of: text) { number, line in
                for link in Self.links(in: line) { links.append((Self.key(of: link.inner), number)) }
            }
            return links
        }, merge: { map, note, links in map.set(note, links: links ?? []) })
    }

    public mutating func remove(_ note: Note) {
        places[note] = nil
    }

    private mutating func set(_ note: Note, links: [(name: String, line: Int)]) {
        guard !links.isEmpty else { return remove(note) }
        places[note] = links.map { link in
            if let id = nameIDs[link.name] { return Place(nameID: id, line: Int32(link.line)) }
            let id = Int32(nameIDs.count)
            nameIDs[link.name] = id
            return Place(nameID: id, line: Int32(link.line))
        }
    }

    /// The links to `note`, in the order of their Notes' paths, then of their lines, each with its
    /// line as it is in the file now. A link to the Note itself is not one, and several on a line
    /// are listed once. Reads the Notes that link here: call it off the main thread.
    public func backlinks(to note: Note, using index: VaultIndex, in vault: Vault) -> [Backlink] {
        let name = Self.name(of: note.path)
        guard let id = nameIDs[name] else { return [] }
        var found: [Backlink] = []
        let sources = places.filter { $0.key != note && $0.value.contains { $0.nameID == id } }.keys
        for source in sources.sorted(by: { $0.path < $1.path }) {
            // A Note changed since it was read has other lines: those that are gone are left out,
            // and the next read puts the map right.
            guard let text = try? vault.text(of: source) else { continue }
            let wanted = Set(places[source, default: []].filter { $0.nameID == id }.map { Int($0.line) })
            MarkdownText.forEachLine(of: text) { number, line in
                guard wanted.contains(number) else { return }
                // The first link on the line that leads here: the line is cut around it.
                let leading = Self.links(in: line).first { link in
                    Self.key(of: link.inner) == name
                        && index.resolve(Wikilink(inner: link.inner).target, from: source) == .note(note)
                }
                if let leading { found.append(Backlink(note: source, line: number, text: Self.shown(line, around: leading.start))) }
            }
        }
        return found
    }

    /// The file name a link asks for, lowercased and without `.md`, as Obsidian takes it.
    private static func key(of inner: String) -> String {
        // The usual link is a plain ASCII name: no Alias, heading, escape or folder. It needs none
        // of Wikilink's Foundation calls, which cost more than reading the line does.
        let plain = inner.utf8.allSatisfy { $0 < 0x80 && $0 != 124 && $0 != 35 && $0 != 92 && $0 != 47 }
        guard plain else { return name(of: Wikilink(inner: inner).target) }
        var utf8 = inner.utf8[...]
        while let first = utf8.first, first == MarkdownText.space || first == MarkdownText.tab { utf8.removeFirst() }
        while let last = utf8.last, last == MarkdownText.space || last == MarkdownText.tab { utf8.removeLast() }
        return trimmingNoteExtension(String(Substring(utf8)).lowercased())
    }

    private static func name(of path: String) -> String {
        trimmingNoteExtension(lastComponent(of: path).lowercased())
    }

    private static func trimmingNoteExtension(_ name: String) -> String {
        guard name.hasSuffix("." + Note.fileExtension) else { return name }
        return String(name.dropLast(Note.fileExtension.count + 1))
    }

    // MARK: Reading the links of a text

    private static let lineWidth = 200

    private static let bracket: UInt8 = 91
    private static let closing: UInt8 = 93

    /// What is between the brackets of each link on a line outside code spans, and where each link
    /// starts, in bytes.
    private static func links(in line: ArraySlice<UInt8>) -> [(inner: String, start: Int)] {
        let bytes = Array(line)
        var links: [(inner: String, start: Int)] = []
        var i = 0
        while i < bytes.count {
            switch bytes[i] {
            case MarkdownText.backslash:
                // An escaped character is text, whatever it is: `\[[x]]` is no link.
                i += 2
            case MarkdownText.backtick:
                let run = bytes[i...].prefix { $0 == MarkdownText.backtick }.count
                // A code span ends at the next run of as many backticks; without one they are text.
                i = MarkdownText.endOfCodeSpan(in: bytes, after: i + run, length: run) ?? i + run
            case bracket where i + 1 < bytes.count && bytes[i + 1] == bracket:
                guard i + 2 < bytes.count, bytes[i + 2] != bracket, let end = closingEnd(in: bytes, from: i + 2) else {
                    i += 1
                    continue
                }
                links.append((String(decoding: bytes[(i + 2)..<end], as: UTF8.self), i))
                i = end + 2
            default:
                i += 1
            }
        }
        return links
    }

    /// Where the `]]` closing a link whose name starts at `start` is, or nil: none on the line,
    /// a backtick first, or nothing between the brackets.
    private static func closingEnd(in bytes: [UInt8], from start: Int) -> Int? {
        var i = start
        while i + 1 < bytes.count {
            if bytes[i] == MarkdownText.backtick { return nil }
            if bytes[i] == closing, bytes[i + 1] == closing { return i == start ? nil : i }
            i += 1
        }
        return nil
    }

    /// The line as the Backlinks list shows it: trimmed, and when long cut to a window around the
    /// link that starts at byte `start`, with `…` where it was cut.
    private static func shown(_ line: ArraySlice<UInt8>, around start: Int) -> String {
        let full = String(decoding: line, as: UTF8.self)
        let text = full.trimmingCharacters(in: .whitespaces)
        guard text.count > lineWidth else { return text }
        // Where the link is among the characters of the trimmed text.
        let leading = full.prefix { $0 == " " || $0 == "\t" }.utf8.count
        let offset = String(decoding: line.prefix(max(0, start - leading)), as: UTF8.self).count
        let from = min(max(0, offset - lineWidth / 3), text.count - lineWidth)
        let window = text.dropFirst(from).prefix(lineWidth)
        return (from > 0 ? "…" : "") + window + (from + lineWidth < text.count ? "…" : "")
    }
}
