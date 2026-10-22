import Foundation
import Testing
import NotekiCore

@Suite("Backlinks")
struct BacklinkTests {
    private struct Fixture {
        let folder: TemporaryVault
        let vault: Vault
        let index: VaultIndex
        var links: LinkMap
    }

    private func fixture(_ files: [String: String]) throws -> Fixture {
        let folder = try TemporaryVault(files: files)
        let vault = Vault(root: folder.url)
        let tree = try vault.tree()
        let index = VaultIndex(tree: tree)
        var links = LinkMap()
        links.rescan(index.notes, in: vault)
        return Fixture(folder: folder, vault: vault, index: index, links: links)
    }

    private func backlinks(_ fixture: Fixture, to path: String) -> [Backlink] {
        fixture.links.backlinks(to: Note(path: path), using: fixture.index, in: fixture.vault)
    }

    @Test("a Note with a Wikilink to the Note is listed with the line that holds it")
    func line() throws {
        let f = try fixture([
            "Home.md": "# Home\n\nSee [[Retro]] for the decisions.\n",
            "Retro.md": "# Retro\n",
        ])

        #expect(backlinks(f, to: "Retro.md") == [Backlink(note: Note(path: "Home.md"), line: 3, text: "See [[Retro]] for the decisions.")])
        #expect(backlinks(f, to: "Home.md").isEmpty)
    }

    @Test("an Alias, a heading, a block and a path all lead to the Note")
    func forms() throws {
        let f = try fixture([
            "A.md": "[[Retro|the retro]]\n[[Retro#Decisions]]\n[[Retro#^block|see]]\n[[Meetings/Retro]]\n[[retro.md]]\n[[Other]]",
            "Meetings/Retro.md": "",
            "Other.md": "",
        ])

        #expect(backlinks(f, to: "Meetings/Retro.md").map(\.line) == [1, 2, 3, 4, 5])
        #expect(backlinks(f, to: "Other.md").map(\.line) == [6])
    }

    @Test("an unlinked mention is not a Backlink")
    func unlinkedMention() throws {
        let f = try fixture(["A.md": "We talked about Retro today, and #Retro too.\n[Retro](Retro.md)\n", "Retro.md": ""])

        #expect(backlinks(f, to: "Retro.md").isEmpty)
    }

    @Test("links in code are not Backlinks")
    func code() throws {
        let f = try fixture([
            "A.md": "`[[B]]`\n``a [[B]] b``\n```\n[[B]]\n```\n~~~swift\n[[B]]\n~~~\nafter [[B]]\n",
            "B.md": "",
        ])

        #expect(backlinks(f, to: "B.md").map(\.line) == [9])
    }

    @Test("a link is a link as the editor reads it: on one line, not empty, not after a third bracket")
    func syntax() throws {
        let f = try fixture([
            "A.md": "[[B\n]]\n[[]]\n[[[B]]\n[[B `x` ]]\n![[B]]\ntext [[B]] and [[B|again]]\n",
            "B.md": "",
        ])

        // `[[[B]]` is a link from its second bracket; an Embed of the Note counts; two on a line count once.
        #expect(backlinks(f, to: "B.md").map(\.line) == [4, 6, 7])
    }

    @Test("a link to itself is not a Backlink of the Note")
    func selfLink() throws {
        let f = try fixture(["A.md": "[[A]] and [[#Heading]] and [[A#Heading]]\n"])

        #expect(backlinks(f, to: "A.md").isEmpty)
    }

    @Test("a name shared by two Notes is a Backlink only of the one the link resolves to from where it is written")
    func sharedName() throws {
        let f = try fixture([
            "Clients/Axo/Kickoff.md": "[[Retro]]\n",
            "Clients/Axo/Retro.md": "",
            "Archive/Retro.md": "",
            "Home.md": "[[Retro]] [[Archive/Retro]]\n",
        ])

        #expect(backlinks(f, to: "Clients/Axo/Retro.md").map(\.note.path) == ["Clients/Axo/Kickoff.md"])
        // From the root the shortest path wins: Archive/Retro.md, whichever way it is written.
        #expect(backlinks(f, to: "Archive/Retro.md").map(\.note.path) == ["Home.md"])
    }

    @Test("the Backlinks come in the order of the Notes' paths, then the lines")
    func order() throws {
        let f = try fixture(["b.md": "x\n[[T]]\n[[T]]", "A.md": "[[T]]", "T.md": ""])

        #expect(backlinks(f, to: "T.md").map { "\($0.note.path):\($0.line)" } == ["A.md:1", "b.md:2", "b.md:3"])
    }

    @Test("lines are counted as the editor counts them, with \\n, \\r\\n and \\r")
    func lines() throws {
        let f = try fixture(["A.md": "one\r\ntwo\r[[T]]\nfour\u{2028}[[T]]\nlast [[T]]", "T.md": ""])

        #expect(backlinks(f, to: "T.md").map(\.line) == [3, 4, 5])
    }

    @Test("a long line is cut around the link, not at its start")
    func longLine() throws {
        let filler = String(repeating: "word ", count: 80)
        let f = try fixture([
            "A.md": "\(filler)[[T]] \(filler)\n[[T]] \(filler)\n\(filler)[[T]]",
            "T.md": "",
        ])

        let found = backlinks(f, to: "T.md")
        #expect(found.count == 3)
        for backlink in found {
            #expect(backlink.text.count <= 202)
            #expect(backlink.text.contains("[[T]]"))
        }
        #expect(found[0].text.hasPrefix("…") && found[0].text.hasSuffix("…"))
        #expect(!found[1].text.hasPrefix("…") && found[1].text.hasSuffix("…"))
        #expect(found[2].text.hasPrefix("…") && !found[2].text.hasSuffix("…"))
    }

    @Test("a link in a table, with its pipe escaped, and a backslash-escaped bracket")
    func escapes() throws {
        let f = try fixture([
            "A.md": "| a | b |\n|---|---|\n| [[T\\|alias]] | x |\n\\[[T]] not a link\n\\\\[[T]] is one\n",
            "T.md": "",
        ])

        #expect(backlinks(f, to: "T.md").map(\.line) == [3, 5])
    }

    @Test("links in the Frontmatter count, as in Obsidian")
    func frontmatter() throws {
        let f = try fixture(["A.md": "---\nrelated: \"[[B]]\"\n---\n", "B.md": ""])

        #expect(backlinks(f, to: "B.md").map(\.line) == [2])
    }

    @Test("rescanning a Note after it changed, and removing one, update its Backlinks")
    func updates() throws {
        var f = try fixture(["A.md": "[[B]]\n", "B.md": "", "C.md": ""])
        #expect(backlinks(f, to: "B.md").map(\.note.path) == ["A.md"])

        try f.folder.write("now [[C]]\n", at: "A.md")
        f.links.rescan([Note(path: "A.md")], in: f.vault)
        #expect(backlinks(f, to: "B.md").isEmpty)
        #expect(backlinks(f, to: "C.md").map(\.note.path) == ["A.md"])

        try f.folder.delete("A.md")
        f.links.rescan([Note(path: "A.md")], in: f.vault)
        #expect(backlinks(f, to: "C.md").isEmpty)
    }

    @Test("a link written before its Note exists becomes a Backlink once the index knows the Note")
    func laterNote() throws {
        var f = try fixture(["A.md": "[[New]]\n"])
        #expect(backlinks(f, to: "New.md").isEmpty)

        try f.folder.write("", at: "New.md")
        let index = VaultIndex(tree: try f.vault.tree())
        f.links.rescan([Note(path: "New.md")], in: f.vault)

        #expect(f.links.backlinks(to: Note(path: "New.md"), using: index, in: f.vault).map(\.note.path) == ["A.md"])
    }

    @Test("a Note that cannot be read is skipped")
    func unreadable() throws {
        let f = try fixture(["A.md": "[[B]]\n", "B.md": ""])
        try Data([0xFF, 0xFE, 0x00]).write(to: f.folder.url.appending(path: "Bad.md"))
        var links = f.links

        links.rescan([Note(path: "Bad.md")], in: f.vault)

        #expect(links.backlinks(to: Note(path: "B.md"), using: f.index, in: f.vault).map(\.note.path) == ["A.md"])
    }

    @Test("the line shown is the one in the file now, and a line that is gone is left out")
    func textNow() throws {
        let f = try fixture(["A.md": "one\ntwo\n[[B]]\n", "B.md": ""])
        try f.folder.write("[[B]] short\n", at: "A.md")

        // The map still holds the old line 3: the file has one line.
        #expect(backlinks(f, to: "B.md").isEmpty)

        var links = f.links
        links.rescan([Note(path: "A.md")], in: f.vault)
        #expect(links.backlinks(to: Note(path: "B.md"), using: f.index, in: f.vault) == [Backlink(note: Note(path: "A.md"), line: 1, text: "[[B]] short")])
    }
}
