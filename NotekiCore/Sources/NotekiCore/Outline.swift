import Foundation

/// A Heading of a Note (CONTEXT.md), as the Outline lists it.
public struct Heading: Equatable, Sendable {
    /// 1 for `#`, 6 for `######`.
    public let level: Int
    /// Without its `#`s, or its underline.
    public let text: String
    /// From 1, counted as the editor counts lines.
    public let line: Int

    public init(level: Int, text: String, line: Int) {
        self.level = level
        self.text = text
        self.line = line
    }

    /// The Headings of a Markdown text, in order: `# Heading` to `###### Heading`, and a line
    /// underlined with `===` or `---`. Not in code, nor in the Frontmatter; an empty one is left out.
    public static func all(in text: String) -> [Heading] {
        var headings: [Heading] = []
        // The paragraph the line before belongs to, which a `===` or `---` line turns into a
        // Heading: where it starts and its first line, enough for the Outline.
        var paragraph: (start: Int, text: String, last: Int)?
        MarkdownText.forEachLine(of: text, skippingFrontmatter: true) { number, bytes in
            let line = String(decoding: bytes, as: UTF8.self)
            let before = paragraph?.last == number - 1 ? paragraph : nil
            paragraph = nil
            if let atx = atx(line) {
                if !atx.text.isEmpty { headings.append(Heading(level: atx.level, text: atx.text, line: number)) }
            } else if let before, let level = underline(line) {
                headings.append(Heading(level: level, text: before.text, line: before.start))
            } else if let before, !line.allSatisfy(\.isWhitespace), !interrupts(line) {
                // A line after a paragraph continues it, however indented.
                paragraph = (before.start, before.text, number)
            } else if startsParagraph(line) {
                paragraph = (number, plain(line.trimmingCharacters(in: .whitespaces)), number)
            }
        }
        return headings
    }

    /// `#` to `######`, indented by at most three spaces, then a space or nothing.
    private static func atx(_ line: String) -> (level: Int, text: String)? {
        let indent = line.prefix { $0 == " " }.count
        guard indent <= 3 else { return nil }
        let rest = line.dropFirst(indent)
        let level = rest.prefix { $0 == "#" }.count
        guard (1...6).contains(level) else { return nil }
        let after = rest.dropFirst(level)
        guard after.isEmpty || after.first == " " || after.first == "\t" else { return nil }
        var text = after.trimmingCharacters(in: .whitespaces)
        // A closing sequence of #s, after a space, is not part of the text.
        let closing = text.reversed().prefix { $0 == "#" }.count
        if closing > 0 {
            let kept = text.dropLast(closing)
            if kept.isEmpty || kept.last == " " || kept.last == "\t" { text = kept.trimmingCharacters(in: .whitespaces) }
        }
        return (level, plain(text))
    }

    /// `===` makes the paragraph above a level 1 Heading, `---` a level 2.
    private static func underline(_ line: String) -> Int? {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        guard line.prefix(while: { $0 == " " }).count <= 3, let mark = trimmed.first else { return nil }
        guard mark == "=" || mark == "-", trimmed.allSatisfy({ $0 == mark }) else { return nil }
        return mark == "=" ? 1 : 2
    }

    /// A line that begins a paragraph: not blank, not indented code, not a rule, a list item or a
    /// quote, which a `---` under it does not make a Heading.
    private static func startsParagraph(_ line: String) -> Bool {
        let indent = line.prefix { $0 == " " }.count
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty, indent <= 3, !line.hasPrefix("\t"), underline(line) == nil else { return false }
        return !interrupts(line)
    }

    /// A quote, a list item or a rule: it ends a paragraph rather than continuing it.
    private static func interrupts(_ line: String) -> Bool {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        if trimmed.first == ">" { return true }
        if trimmed.firstMatch(of: /^([-*+]|\d{1,9}[.)])(\s|$)/) != nil { return true }
        // `***`, `* * *`, `___`: a rule.
        let marks = trimmed.filter { !$0.isWhitespace }
        return marks.count >= 3 && marks.allSatisfy { $0 == marks.first } && "*_-".contains(marks.first!)
    }

    /// A Heading's text as the Outline shows it: a link's text or Alias, without the marks of
    /// emphasis, strikethrough, highlight and code.
    private static func plain(_ text: String) -> String {
        var plain = text.replacing(/\[\[(?:[^\]|]*\|)?([^\]]*)\]\]/) { String($0.1) }
        plain = plain.replacing(/\[([^\]]*)\]\([^)]*\)/) { String($0.1) }
        for mark in ["**", "__", "~~", "==", "`"] { plain = plain.replacing(mark, with: "") }
        return plain.trimmingCharacters(in: .whitespaces)
    }
}

extension OpenNote {
    /// The Headings of the Note as the user has it now, saved or not.
    public var outline: [Heading] { Heading.all(in: text) }
}
