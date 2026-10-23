import Foundation

/// Reading a Markdown text by bytes, as the editor's parser sees it (Editor/src/wikilink.ts):
/// what the scans for links, Tags and Headings share, so that all skip the same code.
enum MarkdownText {
    static let backtick: UInt8 = 96
    static let tilde: UInt8 = 126
    static let space: UInt8 = 32
    static let tab: UInt8 = 9
    static let backslash: UInt8 = 92

    /// Calls `body` with each line and its number, from 1, as CodeMirror counts lines: `\n`, `\r\n`
    /// and `\r` end one. Not given: the lines of a fenced code block and, with `skippingFrontmatter`,
    /// those of the Frontmatter at the top, which `frontmatter` gets first, between its `---` lines.
    static func forEachLine(
        of text: String, skippingFrontmatter: Bool = false, frontmatter: (([String]) -> Void)? = nil,
        _ body: (Int, ArraySlice<UInt8>) -> Void
    ) {
        let bytes = Array(text.utf8)
        let lines = lineRanges(of: bytes)
        let end = skippingFrontmatter || frontmatter != nil ? frontmatterEnd(in: bytes, lines) : nil
        // The Frontmatter's lines, once, for who wants them, before the lines after it.
        if let end { frontmatter?(lines[1..<(end - 1)].map { String(decoding: bytes[$0], as: UTF8.self) }) }
        var number = skippingFrontmatter ? (end ?? 0) : 0
        var fence: (mark: UInt8, length: Int)?
        while number < lines.count {
            let line = bytes[lines[number]]
            number += 1
            if let open = fence {
                if closesFence(line, open.mark, open.length) { fence = nil }
            } else if let opening = fenceOpening(line) {
                fence = opening
            } else {
                body(number, line)
            }
        }
    }

    static func lineRanges(of bytes: [UInt8]) -> [Range<Int>] {
        var lines: [Range<Int>] = []
        // A byte order mark is no part of the first line.
        var start = bytes.starts(with: [0xEF, 0xBB, 0xBF]) ? 3 : 0
        var position = start
        while position < bytes.count {
            if bytes[position] == 10 || bytes[position] == 13 {
                lines.append(start..<position)
                if bytes[position] == 13, position + 1 < bytes.count, bytes[position + 1] == 10 { position += 1 }
                start = position + 1
            }
            position += 1
        }
        lines.append(start..<bytes.count)
        return lines
    }

    /// Every line, code included, as `forEachLine` counts them: the first is line 1.
    static func lines(of text: String) -> [String] {
        let bytes = Array(text.utf8)
        return lineRanges(of: bytes).map { String(decoding: bytes[$0], as: UTF8.self) }
    }

    static let excerptWidth = 200

    /// A line as a list shows it: trimmed, and when long cut to a window around the character at
    /// `offset` (counted in the line as it is), with `…` where it was cut.
    static func excerpt(of line: String, around offset: Int) -> String {
        let text = line.trimmingCharacters(in: .whitespaces)
        guard text.count > excerptWidth else { return text }
        let at = max(0, offset - line.prefix { $0 == " " || $0 == "\t" }.count)
        let from = min(max(0, at - excerptWidth / 3), text.count - excerptWidth)
        let window = text.dropFirst(from).prefix(excerptWidth)
        return (from > 0 ? "…" : "") + window + (from + excerptWidth < text.count ? "…" : "")
    }

    /// How many lines the Frontmatter takes, delimiters included; nil when the Note has none.
    private static func frontmatterEnd(in bytes: [UInt8], _ lines: [Range<Int>]) -> Int? {
        func isDelimiter(_ line: Range<Int>, dots: Bool = false) -> Bool {
            let content = bytes[line]
            let mark: UInt8 = dots ? 46 : 45
            return content.count >= 3 && content.prefix(3).allSatisfy { $0 == mark }
                && content.dropFirst(3).allSatisfy { $0 == space || $0 == tab }
        }
        guard let first = lines.first, isDelimiter(first) else { return nil }
        for (offset, line) in lines.enumerated().dropFirst() where isDelimiter(line) || isDelimiter(line, dots: true) {
            return offset + 1
        }
        return nil
    }

    /// A line of three or more backticks or tildes, indented by at most three spaces; a backtick
    /// fence's info string holds no backtick.
    static func fenceOpening(_ line: ArraySlice<UInt8>) -> (mark: UInt8, length: Int)? {
        let rest = line.drop { $0 == space }
        guard line.count - rest.count <= 3, let mark = rest.first, mark == backtick || mark == tilde else { return nil }
        let length = rest.prefix { $0 == mark }.count
        guard length >= 3 else { return nil }
        if mark == backtick, rest.dropFirst(length).contains(backtick) { return nil }
        return (mark, length)
    }

    static func closesFence(_ line: ArraySlice<UInt8>, _ mark: UInt8, _ length: Int) -> Bool {
        let rest = line.drop { $0 == space }
        guard line.count - rest.count <= 3 else { return false }
        let run = rest.prefix { $0 == mark }.count
        return run >= length && rest.dropFirst(run).allSatisfy { $0 == space || $0 == tab }
    }

    /// Where the code span opened by a run of `length` backticks ends: after the next run of as
    /// many. Nil when there is none: the backticks are text.
    static func endOfCodeSpan(in bytes: [UInt8], after start: Int, length: Int) -> Int? {
        var i = start
        while i < bytes.count {
            guard bytes[i] == backtick else {
                i += 1
                continue
            }
            let run = bytes[i...].prefix { $0 == backtick }.count
            if run == length { return i + run }
            i += run
        }
        return nil
    }
}
