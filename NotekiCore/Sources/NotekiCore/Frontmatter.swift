import Foundation

/// A part of one line of a Note: `line` from 1, counted as the editor counts lines, and the
/// columns in UTF-16 units, as the editor's positions are.
public struct LineSpan: Equatable, Sendable, Encodable {
    public let line: Int
    public let from: Int
    public let to: Int

    public init(line: Int, from: Int, to: Int) {
        self.line = line
        self.from = from
        self.to = to
    }
}

/// One key of the Frontmatter and its value, where they are written.
public struct Property: Equatable, Sendable, Encodable {
    public let key: String
    public let keySpan: LineSpan
    public let value: PropertyValue

    public init(key: String, keySpan: LineSpan, value: PropertyValue) {
        self.key = key
        self.keySpan = keySpan
        self.value = value
    }
}

/// A Property's value as written: a quoted text without its quotes, a list by its items.
public enum PropertyValue: Equatable, Sendable, Encodable {
    case none
    case text(LineSpan)
    case list([LineSpan])

    private enum CodingKeys: String, CodingKey { case kind, span, items }

    /// `{"kind": "text", "span": …}`, as the editor reads it.
    public func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .none:
            try container.encode("none", forKey: .kind)
        case .text(let span):
            try container.encode("text", forKey: .kind)
            try container.encode(span, forKey: .span)
        case .list(let items):
            try container.encode("list", forKey: .kind)
            try container.encode(items, forKey: .items)
        }
    }
}

/// The YAML block at the top of a Note, between `---` lines (SPEC, Frontmatter). Noteki reads the
/// YAML that Obsidian and Takku write: `key: value` at the margin, a value plain or in quotes, a
/// list one item per line or in brackets on one line, blank lines and comments. Anything else
/// (a map inside a Property, a block of text, anchors, a value over several lines, a key twice)
/// is YAML Noteki cannot read: the Note shows it as written, with a mark on the line where
/// reading stopped, and Noteki never rewrites it.
public struct Frontmatter: Equatable, Sendable {
    public enum Reading: Equatable, Sendable, Encodable {
        case properties([Property])
        case unreadable(line: Int)

        private enum CodingKeys: String, CodingKey { case properties, unreadable }

        /// `{"properties": […]}` or `{"unreadable": line}`, as the editor reads it.
        public func encode(to encoder: any Encoder) throws {
            var container = encoder.container(keyedBy: CodingKeys.self)
            switch self {
            case .properties(let properties): try container.encode(properties, forKey: .properties)
            case .unreadable(let line): try container.encode(line, forKey: .unreadable)
            }
        }
    }

    public let reading: Reading

    /// The Frontmatter at the top of a Note's text; nil when there is none, or it never closes.
    public init?(of text: String) {
        guard let (lines, end) = Self.lines(of: Array(text.utf16)) else { return nil }
        reading = Self.read(lines[1..<end])
    }

    private static func read(_ lines: ArraySlice<[UInt16]>) -> Reading {
        var properties: [Property] = []
        var keys: Set<String> = []
        /// The Property with no value just read: the items that follow are its list.
        var listOwner: Int?
        /// How far its first item is indented: a deeper one would continue the item before.
        var itemIndent: Int?
        for (index, units) in zip(lines.indices, lines) {
            let number = index + 1
            let indent = units.prefix { $0 == space || $0 == tab }
            let content = units.dropFirst(indent.count)
            if content.allSatisfy({ $0 == space || $0 == tab }) || content.first == hash { continue }
            if indent.contains(tab) { return .unreadable(line: number) }
            if startsItem(units, at: content.startIndex) {
                guard let owner = listOwner, itemIndent == nil || itemIndent == indent.count,
                      let item = scalar(units, from: content.startIndex + 1, line: number) else {
                    return .unreadable(line: number)
                }
                itemIndent = indent.count
                let property = properties[owner]
                let items = if case .list(let items) = property.value { items } else { [LineSpan]() }
                properties[owner] = Property(key: property.key, keySpan: property.keySpan, value: .list(items + [item]))
                continue
            }
            guard indent.isEmpty, let property = property(units, line: number), keys.insert(property.key).inserted else {
                return .unreadable(line: number)
            }
            properties.append(property)
            listOwner = property.value == .none ? properties.count - 1 : nil
            itemIndent = nil
        }
        return .properties(properties)
    }

    /// `key: value` at the margin.
    private static func property(_ units: [UInt16], line: Int) -> Property? {
        guard let first = units.first, !indicators.contains(first) else { return nil }
        guard let colon = units.indices.first(where: { units[$0] == colon && (units.count == $0 + 1 || units[$0 + 1] == space) })
        else { return nil }
        var keyEnd = colon
        while keyEnd > 0, units[keyEnd - 1] == space { keyEnd -= 1 }
        guard keyEnd > 0 else { return nil }
        let key = String(decoding: units[..<keyEnd], as: UTF16.self)
        let keySpan = LineSpan(line: line, from: 0, to: keyEnd)
        let start = skipSpaces(units, from: colon + 1)
        if start == units.count || units[start] == hash {
            return Property(key: key, keySpan: keySpan, value: .none)
        }
        if units[start] == openBracket {
            guard let items = flowList(units, from: start, line: line) else { return nil }
            return Property(key: key, keySpan: keySpan, value: .list(items))
        }
        guard let value = scalar(units, from: start, line: line) else { return nil }
        return Property(key: key, keySpan: keySpan, value: .text(value))
    }

    /// A value alone on the rest of its line, from `start`: quoted, or plain up to a comment.
    private static func scalar(_ units: [UInt16], from start: Int, line: Int) -> LineSpan? {
        let start = skipSpaces(units, from: start)
        guard start < units.count, units[start] != hash else { return LineSpan(line: line, from: start, to: start) }
        if let (span, end) = quoted(units, from: start, line: line) {
            return endsLine(units, from: end) ? span : nil
        }
        // `- b` after a key is a list where YAML allows none.
        guard !indicators.contains(units[start]), !startsItem(units, at: start) else { return nil }
        var end = start
        while end < units.count, !(units[end] == hash && (units[end - 1] == space || units[end - 1] == tab)) { end += 1 }
        while end > start, units[end - 1] == space || units[end - 1] == tab { end -= 1 }
        if isMap(units[start..<end]) { return nil }
        return LineSpan(line: line, from: start, to: end)
    }

    /// `[a, "b, c", 'd']` on one line: its items.
    private static func flowList(_ units: [UInt16], from start: Int, line: Int) -> [LineSpan]? {
        var items: [LineSpan] = []
        var at = skipSpaces(units, from: start + 1)
        if at < units.count, units[at] == closeBracket { return endsLine(units, from: at + 1) ? [] : nil }
        while at < units.count {
            if let (span, end) = quoted(units, from: at, line: line) {
                items.append(span)
                at = skipSpaces(units, from: end)
            } else {
                var end = at
                while end < units.count, units[end] != comma, units[end] != closeBracket {
                    if indicators.contains(units[end]) && end == at || units[end] == openBracket || units[end] == openBrace { return nil }
                    end += 1
                }
                var trimmed = end
                while trimmed > at, units[trimmed - 1] == space { trimmed -= 1 }
                guard trimmed > at, !isMap(units[at..<trimmed]) else { return nil }
                items.append(LineSpan(line: line, from: at, to: trimmed))
                at = end
            }
            guard at < units.count else { return nil }
            if units[at] == closeBracket { return endsLine(units, from: at + 1) ? items : nil }
            guard units[at] == comma else { return nil }
            at = skipSpaces(units, from: at + 1)
            // A comma before the closing bracket ends no item.
            if at < units.count, units[at] == closeBracket, !items.isEmpty { return endsLine(units, from: at + 1) ? items : nil }
        }
        return nil
    }

    /// A text in double quotes (`\"` inside) or single quotes (`''` inside) closed on its line:
    /// what is between the quotes, and where the closing quote ends.
    private static func quoted(_ units: [UInt16], from start: Int, line: Int) -> (LineSpan, Int)? {
        let quote = units[start]
        guard quote == doubleQuote || quote == singleQuote else { return nil }
        var at = start + 1
        while at < units.count {
            if quote == doubleQuote, units[at] == backslash {
                at += 2
                continue
            }
            if units[at] == quote {
                if quote == singleQuote, at + 1 < units.count, units[at + 1] == singleQuote {
                    at += 2
                    continue
                }
                return (LineSpan(line: line, from: start + 1, to: at), at + 1)
            }
            at += 1
        }
        return nil
    }

    /// `a: b`, or `a:` at the end: a map, not a text.
    private static func isMap(_ value: ArraySlice<UInt16>) -> Bool {
        value.last == colon || zip(value, value.dropFirst()).contains { $0 == colon && ($1 == space || $1 == tab) }
    }

    /// A dash and a space, or a dash alone: an item of a list.
    private static func startsItem(_ units: [UInt16], at start: Int) -> Bool {
        units[start] == dash && (start + 1 == units.count || units[start + 1] == space || units[start + 1] == tab)
    }

    /// Nothing but spaces and a comment from `start` to the end of the line.
    private static func endsLine(_ units: [UInt16], from start: Int) -> Bool {
        let rest = skipSpaces(units, from: start)
        return rest == units.count || units[rest] == hash && rest > start
    }

    private static func skipSpaces(_ units: [UInt16], from start: Int) -> Int {
        var at = start
        while at < units.count, units[at] == space || units[at] == tab { at += 1 }
        return at
    }

    /// The Note's lines in UTF-16, and the index of the Frontmatter's closing line; nil when the
    /// Note has no Frontmatter or it never closes.
    private static func lines(of units: [UInt16], _ ranges: [Range<Int>]? = nil) -> (lines: [[UInt16]], end: Int)? {
        let lines = (ranges ?? lineRanges(of: units)).map { Array(units[$0]) }
        guard let first = lines.first, isDelimiter(first, dots: false),
              let end = lines.indices.dropFirst().first(where: { isDelimiter(lines[$0], dots: true) })
        else { return nil }
        return (lines, end)
    }

    /// Where each line is, its line end left out, as CodeMirror splits them: `\n`, `\r\n` and `\r`
    /// end one.
    private static func lineRanges(of units: [UInt16]) -> [Range<Int>] {
        var ranges: [Range<Int>] = []
        var start = 0
        var at = 0
        while at < units.count {
            let unit = units[at]
            if unit == newline || unit == carriageReturn {
                ranges.append(start..<at)
                at += unit == carriageReturn && at + 1 < units.count && units[at + 1] == newline ? 2 : 1
                start = at
            } else {
                at += 1
            }
        }
        ranges.append(start..<units.count)
        return ranges
    }

    /// `---`, or with `dots` also `...`, and spaces after it.
    private static func isDelimiter(_ line: [UInt16], dots: Bool) -> Bool {
        guard line.count >= 3 else { return false }
        let mark = line[0]
        guard mark == dash || dots && mark == dot, line.prefix(3).allSatisfy({ $0 == mark }) else { return false }
        return line.dropFirst(3).allSatisfy { $0 == space || $0 == tab }
    }

    private static let space: UInt16 = 32, tab: UInt16 = 9, newline: UInt16 = 10, carriageReturn: UInt16 = 13
    private static let hash: UInt16 = 35, dash: UInt16 = 45, dot: UInt16 = 46, colon: UInt16 = 58, comma: UInt16 = 44
    private static let doubleQuote: UInt16 = 34, singleQuote: UInt16 = 39, backslash: UInt16 = 92
    private static let openBracket: UInt16 = 91, closeBracket: UInt16 = 93, openBrace: UInt16 = 123
    /// What starts YAML that is not a plain text: maps, blocks of text, anchors, types, directives.
    private static let indicators: Set<UInt16> = Set("{}[]|>&*!%@`\"'?,".utf16)
}

/// A Tag added to the Note's `tags` Property, or removed from it.
public enum TagEdit: Equatable, Sendable {
    case add(String)
    case remove(String)
}

extension Frontmatter {
    /// The Note's text with its `tags` Property edited, written as a YAML list one item per line,
    /// without quotes or `#` (SPEC, Frontmatter): only those lines change, Tags in the text never.
    /// Without a `tags` Property, one is added at the end of the Frontmatter. Nil when nothing
    /// changes: a Tag already there in any case, one not there to remove, no Tag at all, no
    /// Frontmatter, YAML Noteki cannot read, or Tags it could not write back as they are; it
    /// never rewrites those.
    public static func editingTags(_ edit: TagEdit, in text: String) -> String? {
        var units = Array(text.utf16)
        let ranges = lineRanges(of: units)
        guard let (lines, end) = lines(of: units, ranges), case .properties(let properties) = read(lines[1..<end]) else { return nil }
        let property = properties.first { $0.key == "tags" }
        let cut = { (span: LineSpan) in String(decoding: lines[span.line - 1][span.from..<span.to], as: UTF16.self) }
        var tags: [String] = switch property?.value {
        case .text(let span)?: cut(span).split { $0 == "," || $0 == " " || $0 == "\t" }.map(String.init)
        case .list(let items)?: items.map(cut)
        case .none?, nil: []
        }
        tags = tags.map(withoutHash).filter { !$0.isEmpty }
        // Written back plain, a Tag in quotes for what it holds (`"a #b"`, `"x: y"`, `"null"`)
        // would read otherwise, and a comment among its lines would be lost: such a Property is
        // left alone.
        guard tags.allSatisfy(isWritable), !(property.map { hasComment(property: $0, lines) } ?? false) else { return nil }
        switch edit {
        case .add(let tag):
            let tag = withoutHash(tag)
            guard isWritable(tag), !tags.contains(where: { $0.lowercased() == tag.lowercased() }) else { return nil }
            tags.append(tag)
        case .remove(let tag):
            let tag = withoutHash(tag).lowercased()
            guard tags.contains(where: { $0.lowercased() == tag }) else { return nil }
            tags.removeAll { $0.lowercased() == tag }
        }
        let lineEnd = text.contains("\r\n") ? "\r\n" : text.contains("\r") ? "\r" : "\n"
        let written = (tags.isEmpty ? ["tags: []"] : ["tags:"] + tags.map { "  - \($0)" }).joined(separator: lineEnd)
        if let property {
            // From the key's line to its last item's.
            var last = property.keySpan.line
            if case .list(let items) = property.value, let item = items.last { last = item.line }
            units.replaceSubrange(ranges[property.keySpan.line - 1].lowerBound..<ranges[last - 1].upperBound, with: written.utf16)
        } else {
            units.insert(contentsOf: (written + lineEnd).utf16, at: ranges[end].lowerBound)
        }
        return String(decoding: units, as: UTF16.self)
    }

    /// A Tag YAML reads back as it is when written plain: not `null`, `true` or `false`.
    private static func isWritable(_ tag: String) -> Bool {
        TagMap.validTag(tag) == tag && !["null", "true", "false"].contains(tag.lowercased())
    }

    /// Whether a comment is among the lines of `property`, from its key to its last item: outside
    /// its values, a `#` after a space or starting a line.
    private static func hasComment(property: Property, _ lines: [[UInt16]]) -> Bool {
        var spans = [property.keySpan]
        switch property.value {
        case .none: break
        case .text(let span): spans.append(span)
        case .list(let items): spans += items
        }
        let last = spans.map(\.line).max() ?? property.keySpan.line
        for number in property.keySpan.line...last {
            var units = lines[number - 1]
            for span in spans where span.line == number {
                units.replaceSubrange(span.from..<span.to, with: repeatElement(UInt16(120), count: span.to - span.from))
            }
            for (index, unit) in units.enumerated() where unit == hash {
                if index == 0 || units[index - 1] == space || units[index - 1] == tab { return true }
            }
        }
        return false
    }

    private static func withoutHash(_ tag: String) -> String {
        tag.hasPrefix("#") ? String(tag.dropFirst()) : tag
    }
}
