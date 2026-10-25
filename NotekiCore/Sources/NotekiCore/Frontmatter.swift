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
        let lines = Self.lines(of: text)
        guard let first = lines.first, Self.isDelimiter(first, dots: false),
              let end = lines.indices.dropFirst().first(where: { Self.isDelimiter(lines[$0], dots: true) })
        else { return nil }
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

    /// The Note's lines in UTF-16, as CodeMirror splits them: `\n`, `\r\n` and `\r` end one.
    private static func lines(of text: String) -> [[UInt16]] {
        var lines: [[UInt16]] = [[]]
        var previous: UInt16 = 0
        for unit in text.utf16 {
            if unit == newline, previous == carriageReturn {
                previous = unit
                continue
            }
            if unit == newline || unit == carriageReturn {
                lines.append([])
            } else {
                lines[lines.count - 1].append(unit)
            }
            previous = unit
        }
        return lines
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
