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

@Suite("Writing Tags")
struct TagWritingTests {
    @Test("an added Tag ends the `tags` list, one item per line; nothing else changes, Tags in the text least of all")
    func add() {
        let text = "---\ntitle: x\ntags:\n  - meeting\ndate: 1\n---\nBody #inline\n"

        #expect(Frontmatter.editingTags(.add("client/axo"), in: text) == "---\ntitle: x\ntags:\n  - meeting\n  - client/axo\ndate: 1\n---\nBody #inline\n")
    }

    @Test("Tags written otherwise become a list one item per line, without quotes or #")
    func rewritten() {
        #expect(Frontmatter.editingTags(.add("new"), in: "---\ntags: [meeting, \"b\"]\n---\n") == "---\ntags:\n  - meeting\n  - b\n  - new\n---\n")
        #expect(Frontmatter.editingTags(.add("new"), in: "---\ntags: a, b\n---\n") == "---\ntags:\n  - a\n  - b\n  - new\n---\n")
        #expect(Frontmatter.editingTags(.add("new"), in: "---\ntags: meeting\n---\n") == "---\ntags:\n  - meeting\n  - new\n---\n")
        #expect(Frontmatter.editingTags(.add("new"), in: "---\ntags:\nx: y\n---\n") == "---\ntags:\n  - new\nx: y\n---\n")
    }

    @Test("without a `tags` Property, one is added at the end of the Frontmatter")
    func noTags() {
        #expect(Frontmatter.editingTags(.add("new"), in: "---\ntitle: x\n---\nBody") == "---\ntitle: x\ntags:\n  - new\n---\nBody")
        #expect(Frontmatter.editingTags(.add("new"), in: "---\n---\nBody") == "---\ntags:\n  - new\n---\nBody")
    }

    @Test("a removed Tag leaves the others; the last one leaves an empty list")
    func remove() {
        #expect(Frontmatter.editingTags(.remove("b"), in: "---\ntags:\n  - a\n  - B\nx: y\n---\n") == "---\ntags:\n  - a\nx: y\n---\n")
        #expect(Frontmatter.editingTags(.remove("#a"), in: "---\ntags: [a]\n---\n") == "---\ntags: []\n---\n")
    }

    @Test("the lines after the last item stay; blank lines between the items go")
    func around() {
        let text = "---\ntags:\n  - a\n\n  - b\n\n# end\nx: y\n---\n"

        #expect(Frontmatter.editingTags(.remove("a"), in: text) == "---\ntags:\n  - b\n\n# end\nx: y\n---\n")
    }

    @Test("a comment among the Tags' lines would be lost: such a Property is left alone")
    func comments() {
        #expect(Frontmatter.editingTags(.add("c"), in: "---\ntags: [a, b] # keep\n---\n") == nil)
        #expect(Frontmatter.editingTags(.add("c"), in: "---\ntags: alpha #beta\n---\n") == nil)
        #expect(Frontmatter.editingTags(.add("c"), in: "---\ntags: # mine\n  - a\n---\n") == nil)
        #expect(Frontmatter.editingTags(.remove("a"), in: "---\ntags:\n  - a\n  # old\n  - b\n---\n") == nil)
    }

    @Test("a word YAML reads as no text, written bare, is not added; a list holding one is left alone")
    func reserved() {
        #expect(Frontmatter.editingTags(.add("null"), in: "---\ntags: [a]\n---\n") == nil)
        #expect(Frontmatter.editingTags(.add("True"), in: "---\ntags: [a]\n---\n") == nil)
        #expect(Frontmatter.editingTags(.add("c"), in: "---\ntags: [\"false\"]\n---\n") == nil)
    }

    @Test("a Tag spelled as one already there, one not there to remove, or no Tag at all changes nothing")
    func nothing() {
        let text = "---\ntags: [Meeting]\n---\n"

        #expect(Frontmatter.editingTags(.add("meeting"), in: text) == nil)
        #expect(Frontmatter.editingTags(.add("#Meeting"), in: text) == nil)
        #expect(Frontmatter.editingTags(.remove("other"), in: text) == nil)
        #expect(Frontmatter.editingTags(.add("two words"), in: text) == nil)
        #expect(Frontmatter.editingTags(.add("1abc"), in: text) == nil)
        #expect(Frontmatter.editingTags(.add(""), in: text) == nil)
        #expect(Frontmatter.editingTags(.add("#New"), in: text) == "---\ntags:\n  - Meeting\n  - New\n---\n")
    }

    @Test("Tags Noteki could not write back as they are, in quotes for what they hold, are never rewritten")
    func quoted() {
        #expect(Frontmatter.editingTags(.add("c"), in: "---\ntags: [\"a #b\"]\n---\n") == nil)
        #expect(Frontmatter.editingTags(.add("c"), in: "---\ntags:\n  - \"x: y\"\n---\n") == nil)
        #expect(Frontmatter.editingTags(.add("c"), in: "---\ntags:\n  - two words\n---\n") == nil)
        #expect(Frontmatter.editingTags(.add("c"), in: "---\ntags: [\"client/axo\"]\n---\n") == "---\ntags:\n  - client/axo\n  - c\n---\n")
    }

    @Test("YAML Noteki cannot read, or no Frontmatter, is never rewritten")
    func unreadable() {
        #expect(Frontmatter.editingTags(.add("x"), in: "---\ntags: [a]\nb:\n  c: d\n---\n") == nil)
        #expect(Frontmatter.editingTags(.add("x"), in: "# Note\n") == nil)
    }

    @Test("the Note's line ends are kept")
    func lineEnds() {
        #expect(Frontmatter.editingTags(.add("b"), in: "---\r\ntags: [a]\r\n---\r\nBody") == "---\r\ntags:\r\n  - a\r\n  - b\r\n---\r\nBody")
    }
}
