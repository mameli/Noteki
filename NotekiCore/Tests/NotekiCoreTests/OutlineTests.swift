import Testing
import NotekiCore

@Suite("Outline")
struct OutlineTests {
    private func outline(_ text: String) -> [String] {
        Heading.all(in: text).map { "\($0.level) \($0.text) @\($0.line)" }
    }

    @Test("# to ###### are the levels, with the line each is on")
    func levels() {
        #expect(outline("# One\ntext\n## Two\n### Three\n#### Four\n##### Five\n###### Six\n####### Seven\n") == [
            "1 One @1", "2 Two @3", "3 Three @4", "4 Four @5", "5 Five @6", "6 Six @7",
        ])
    }

    @Test("a Heading needs a space after its #s; closing #s and spaces are not its text; an empty one is left out")
    func syntax() {
        #expect(outline("#tag\n#\n## Closed ##\n   ### Indented\n    #### Code\n## Hash#inside #\n\\# Escaped\n") == [
            "2 Closed @3", "3 Indented @4", "2 Hash#inside @6",
        ])
    }

    @Test("a line underlined with = or - is a Heading too, a - line after a blank is a rule")
    func setext() {
        #expect(outline("Title\n=====\n\nSub\n---\n\n---\ntext\n") == ["1 Title @1", "2 Sub @4"])
    }

    @Test("Headings in code and in the Frontmatter are not Headings")
    func code() {
        #expect(outline("---\ntitle: x\n---\n# Real\n```\n# Not\n```\n~~~\n## Not\n~~~\n## Also real\n") == [
            "1 Real @4", "2 Also real @11",
        ])
    }

    @Test("a rule after a fence is no Heading of the line before the fence")
    func setextAfterFence() {
        #expect(outline("text\n```\ncode\n```\n---\n") == [])
    }

    @Test("lines are counted as the editor counts them")
    func lineEndings() {
        #expect(outline("a\r\n# B\rc\n## D") == ["1 B @2", "2 D @4"])
    }

    @Test("a paragraph of several lines underlined is one Heading, at its first line")
    func multilineSetext() {
        #expect(outline("First\nSecond\n---\n\ntext\n    lazy\n===\n") == ["2 First @1", "1 text @5"])
    }

    @Test("a rule under a list item, a quote or another rule is no Heading")
    func notSetext() {
        #expect(outline("- item\n---\n> quote\n---\n***\n---\n1. one\n---\ntext\n- item\n---\n") == [])
    }

    @Test("a Heading's text is shown without its links' and emphasis' marks")
    func text() {
        #expect(outline("# See [[Folder/Note|the alias]] and [[Other]]\n## **Bold** ~~old~~ `code` [site](https://x.org) ==mark==\n") == [
            "1 See the alias and Other @1", "2 Bold old code site mark @2",
        ])
    }

    @Test("an open Note gives its Outline")
    func openNote() throws {
        let folder = try TemporaryVault(files: ["a.md": "# One\n"])
        var open = try Vault(root: folder.url).open(Note(path: "a.md"))
        open.edit("# One\n## Two\n")

        #expect(open.outline.map(\.text) == ["One", "Two"])
    }
}
