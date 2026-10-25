import Foundation
import Testing
import NotekiCore

@Suite("Tags")
struct TagTests {
    private func map(_ files: [String: String]) throws -> (TagMap, TemporaryVault) {
        let folder = try TemporaryVault(files: files)
        let vault = Vault(root: folder.url)
        var tags = TagMap()
        tags.rescan(VaultIndex(tree: try vault.tree()).notes, in: vault)
        return (tags, folder)
    }

    private func names(_ nodes: [TagNode]) -> [String] {
        nodes.map { "\($0.name):\($0.noteCount)" }
    }

    @Test("the tags Property is read as a list, a flow list, a single value and a comma list, with or without #")
    func frontmatter() throws {
        let (tags, _) = try map([
            "block.md": "---\ntags:\n  - alpha\n  - \"#beta\"\n  - 'gamma'\n---\n",
            "flow.md": "---\ntags: [delta, #epsilon, \"zeta\"]\n---\n",
            "single.md": "---\ntags: eta\n---\n",
            "comma.md": "---\ntags: theta, #iota kappa\n---\n",
            "flush.md": "---\ntags:\n- lambda\n---\n",
        ])

        #expect(names(tags.tree()) == [
            "alpha:1", "beta:1", "delta:1", "epsilon:1", "eta:1", "gamma:1", "iota:1", "kappa:1", "lambda:1", "theta:1", "zeta:1",
        ])
    }

    @Test("the Tags offered to add one: every level as most Notes write it, those most Notes have first")
    func completions() throws {
        let (tags, _) = try map([
            "a.md": "#Client/axo #meeting",
            "b.md": "#client/bnp #meeting",
            "c.md": "#client/axo",
        ])

        #expect(tags.completions() == ["client", "client/axo", "meeting", "client/bnp"])
    }

    @Test("only the tags Property counts, and only in the Frontmatter at the top")
    func onlyTags() throws {
        let (tags, _) = try map([
            "a.md": "---\ntitle: tags\naliases: [nope]\ntags: [yes]\nauthor: x\n---\ntags: [no]\n",
            "b.md": "text\n---\ntags: [no]\n---\n",
            "c.md": "---\ntags: [unclosed\n",
            "d.md": "---\ntags: [a, b]\nother: [c]\n---\n",
        ])

        #expect(names(tags.tree()) == ["a:1", "b:1", "yes:1"])
    }

    @Test("a #tag in the text starts with a letter, follows a space or the line's start, and ends at a character a tag cannot hold")
    func text() throws {
        let (tags, _) = try map([
            "a.md": "# Heading\n\nA #one, #two/sub and #three_3-x. #4ever #-no # no\n#four at the start\nurl.com/#frag [[Note#heading]] a#b (#paren)\n",
        ])

        #expect(names(tags.tree()) == ["four:1", "one:1", "three_3-x:1", "two:1"])
        #expect(tags.tree().first { $0.name == "two" }?.children.map(\.name) == ["sub"])
    }

    @Test("a # in code, in a fence, escaped or in the Frontmatter's comments is not a tag")
    func notInCode() throws {
        let (tags, _) = try map([
            "a.md": "---\n# comment #fm\n---\n`#inline` and ``#double`` and \\#escaped\n```\n#fenced\n```\n~~~\n#tilde\n~~~\n#real\n",
        ])

        #expect(names(tags.tree()) == ["real:1"])
    }

    @Test("accents, other scripts and emoji are letters")
    func unicode() throws {
        let (tags, _) = try map(["a.md": "#città #日本 #été/été\n"])

        #expect(names(tags.tree()) == ["città:1", "été:1", "日本:1"])
    }

    @Test("Tags are nested at every /, and a Note counts once for a Tag and its sub-Tags")
    func nesting() throws {
        let (tags, _) = try map([
            "a.md": "---\ntags: [client/axo, client/bnp]\n---\n",
            "b.md": "#client/axo/2026 and #client\n",
            "c.md": "#other\n",
        ])
        let tree = tags.tree()

        #expect(names(tree) == ["client:2", "other:1"])
        let client = try #require(tree.first)
        #expect(names(client.children) == ["axo:2", "bnp:1"])
        #expect(client.path == "client")
        #expect(client.children[0].path == "client/axo")
        #expect(names(client.children[0].children) == ["2026:1"])
        #expect(client.children[0].children[0].path == "client/axo/2026")
    }

    @Test("choosing a Tag lists the Notes of it and of its sub-Tags, by path")
    func notesOfATag() throws {
        let (tags, _) = try map([
            "z.md": "#client/axo\n",
            "Meetings/a.md": "#client\n",
            "b.md": "#client/bnp #client/bnp\n",
            "c.md": "#clients\n",
        ])

        #expect(tags.notes(tagged: "client").map(\.path) == ["Meetings/a.md", "b.md", "z.md"])
        #expect(tags.notes(tagged: "client/axo").map(\.path) == ["z.md"])
        #expect(tags.notes(tagged: "clients").map(\.path) == ["c.md"])
        #expect(tags.notes(tagged: "missing").isEmpty)
    }

    @Test("upper and lower case are one Tag, shown as most Notes write it")
    func caseInsensitive() throws {
        let (tags, _) = try map([
            "a.md": "#Client/Axo\n", "b.md": "#client/axo\n", "c.md": "#client/axo\n", "d.md": "#CLIENT\n",
        ])

        #expect(names(tags.tree()) == ["client:4"])
        #expect(names(tags.tree()[0].children) == ["axo:3"])
        #expect(tags.notes(tagged: "client/axo").count == 3)
        #expect(tags.notes(tagged: "Client").count == 4)
    }

    @Test("a Note that changed is read again, and one deleted or unreadable has no Tags")
    func updates() throws {
        var (tags, folder) = try map(["a.md": "#one\n", "b.md": "#two\n"])
        let vault = Vault(root: folder.url)

        try folder.write("#three\n", at: "a.md")
        tags.rescan([Note(path: "a.md")], in: vault)
        #expect(names(tags.tree()) == ["three:1", "two:1"])

        try folder.delete("b.md")
        tags.rescan([Note(path: "b.md")], in: vault)
        #expect(names(tags.tree()) == ["three:1"])

        tags.remove(Note(path: "a.md"))
        #expect(tags.tree().isEmpty)
    }

    @Test("a Tag written twice in a Note counts once, in the Frontmatter and the text")
    func once() throws {
        let (tags, _) = try map(["a.md": "---\ntags: [x]\n---\n#x #X\n"])

        #expect(names(tags.tree()) == ["x:1"])
    }

    @Test("a comment after a value is not a Tag, and a level left empty ends the Tag")
    func comments() throws {
        let (tags, _) = try map([
            "a.md": "---\ntags:\n  - alpha # old\n  - #beta\ntags2: x\n---\n",
            "b.md": "---\ntags: gamma # comment\nother: x\n---\n", "n.md": "---\ntags: null\n---\n",
            "c.md": "#delta//no and #e/f/\n",
        ])

        #expect(names(tags.tree()) == ["alpha:1", "beta:1", "delta:1", "e:1", "gamma:1"])
        #expect(tags.tree().first { $0.name == "e" }?.children.map(\.name) == ["f"])
    }

    @Test("a blank line does not end a block list, a byte order mark does not hide the Frontmatter")
    func yamlDetails() throws {
        let (tags, _) = try map([
            "a.md": "---\ntags:\n  - one\n\n  - two\nother: x\n---\n",
            "b.md": "\u{FEFF}---\ntags: [three]\n---\n#four\n",
            "c.md": "---\r\ntags: [five]\r\n---\r\n#six\r\n",
        ])

        #expect(names(tags.tree()) == ["five:1", "four:1", "one:1", "six:1", "three:1", "two:1"])
    }

    @Test("digits are digits, not superscripts or fractions")
    func digits() throws {
        let (tags, _) = try map(["a.md": "#a2 #b² #c½ #d٣\n"])

        #expect(names(tags.tree()) == ["a2:1", "b:1", "c:1", "d٣:1"])
    }
}
