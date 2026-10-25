import Testing
import NotekiCore

@Suite("Frontmatter")
struct FrontmatterTests {
    /// The Properties as a list shows them, their values cut from the text where the spans say;
    /// or the line Noteki cannot read.
    private func read(_ text: String) -> [String]? {
        guard let frontmatter = Frontmatter(of: text) else { return nil }
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false).map { Array($0.utf16) }
        func cut(_ span: LineSpan) -> String {
            String(decoding: lines[span.line - 1][span.from..<span.to], as: UTF16.self)
        }
        switch frontmatter.reading {
        case .unreadable(let line):
            return ["unreadable at \(line)"]
        case .properties(let properties):
            return properties.map { property in
                #expect(cut(property.keySpan) == property.key)
                switch property.value {
                case .none: return property.key
                case .text(let span): return "\(property.key) = \(cut(span))"
                case .list(let items): return "\(property.key): " + items.map(cut).joined(separator: " | ")
                }
            }
        }
    }

    @Test("Properties in order, each value as written; a quoted one without its quotes, a comment left out")
    func values() {
        let text = """
        ---
        takku_id: B92F454D
        date: 2026-10-25T13:14
        title: "Guida: Dominio 1 \\"x\\" (27%)"
        quote: 'it''s'
        empty:
        language: it # Italian
        tabbed: x\t# a comment after a tab
        ---
        # Text
        """
        #expect(read(text) == [
            "takku_id = B92F454D", "date = 2026-10-25T13:14", #"title = Guida: Dominio 1 \"x\" (27%)"#,
            "quote = it''s", "empty", "language = it", "tabbed = x",
        ])
    }

    @Test("a list one item per line, blank lines and comments between them, or on one line in brackets")
    func lists() {
        let text = """
        ---
        tags:
          - meeting

          # old
          - "client/axo"
        sources:
        - "Video: https://example.com/a, b"
        - plain
        inline: [meeting, "a, b", 'c', d e]
        none: []
        trailing: [x, ]
        ---
        """
        #expect(read(text) == [
            "tags: meeting | client/axo", "sources: Video: https://example.com/a, b | plain",
            "inline: meeting | a, b | c | d e", "none: ", "trailing: x",
        ])
    }

    @Test("no Frontmatter, or one never closed, is none; an empty one has no Properties")
    func none() {
        #expect(read("# Title\n---\na: b\n---") == nil)
        #expect(read("---\na: b\n") == nil)
        #expect(read("---\n---\ntext") == [])
    }

    @Test("what Noteki cannot read is the line where it stops", arguments: [
        ("---\na: b\nc:\n  d: e\n---", 4),        // a map inside a Property
        ("---\na: |\n  text\n---", 2),             // a block of text
        ("---\na: {b: c}\n---", 2),                // a map in braces
        ("---\na: \"open\n---", 2),                // a quote not closed
        ("---\na: [b,\n  c]\n---", 2),             // a list in brackets over lines
        ("---\na: b\n  more\n---", 3),             // a value continued on the next line
        ("---\na: b\n- c\n---", 3),                // an item with no list to belong to
        ("---\na: b\na: c\n---", 3),               // a Property twice
        ("---\na:\n\t- b\n---", 3),                // a tab before an item
        ("---\na:\n  - b: c\n---", 3),             // a map as an item
        ("---\na: b: c\n---", 2),                  // a colon and a space in a value
        ("---\njust text\n---", 2),                // no key
        ("---\n\"a\": b\n---", 2),                 // a key in quotes
        ("---\na: &anchor b\n---", 2),             // YAML's anchors and types
        ("---\na: \"b\" c\n---", 2),               // text after a quote
        ("---\na: - b\n---", 2),                  // an item after a key
        ("---\na: [x: y]\n---", 2),               // a map as an item in brackets
        ("---\na:\n  - b\n    - c\n---", 4),       // an item continued on the next line
        ("---\n: x\n---", 2),                     // no key before the colon
    ])
    func unreadable(text: String, line: Int) {
        #expect(read(text) == ["unreadable at \(line)"])
    }

    @Test("lines are counted as the editor counts them, columns in UTF-16 as its positions")
    func positions() throws {
        let frontmatter = try #require(Frontmatter(of: "---\r\ntitle: \"café 🎉\"\r\nx: y\r\n---"))
        #expect(frontmatter.reading == .properties([
            Property(key: "title", keySpan: LineSpan(line: 2, from: 0, to: 5), value: .text(LineSpan(line: 2, from: 8, to: 15))),
            Property(key: "x", keySpan: LineSpan(line: 3, from: 0, to: 1), value: .text(LineSpan(line: 3, from: 3, to: 4))),
        ]))
    }
}
