import Foundation
import Testing
import NotekiCore

@Suite("Search")
struct SearchTests {
    private func search(_ query: String, in files: [String: String]) throws -> [SearchResult] {
        let folder = try TemporaryVault(files: files)
        let vault = Vault(root: folder.url)
        let notes = VaultIndex(tree: try vault.tree()).notes
        return vault.search(SearchQuery(query), in: notes).results
    }

    @Test("every word must be in the Note, in any order and on any line, case and accents ignored")
    func words() throws {
        let files = [
            "a.md": "# Budget\n\nThe CAFÉ order was late.\n",
            "b.md": "late budget",
            "c.md": "budget only",
        ]

        #expect(try search("late budget", in: files).map(\.note.path) == ["a.md", "b.md"])
        #expect(try search("cafe", in: files).map(\.note.path) == ["a.md"])
        #expect(try search("Café ORDER", in: files).map(\.note.path) == ["a.md"])
        #expect(try search("missing budget", in: files).isEmpty)
        #expect(try search("   ", in: files).isEmpty)
    }

    @Test("each Note comes with the lines that hold a word, numbered as the editor counts them")
    func lines() throws {
        let results = try search("alpha beta", in: ["a.md": "alpha here\r\nnothing\nbeta and Alpha\rlast"])

        #expect(results.first?.lines == [
            SearchLine(line: 1, text: "alpha here"),
            SearchLine(line: 3, text: "beta and Alpha"),
        ])
    }

    @Test("a word in the Note's name counts, and Notes whose name holds every word come first")
    func names() throws {
        let results = try search("retro", in: [
            "Archive/a.md": "the retro went well",
            "Meetings/Retro 2026.md": "# Notes",
            "b.md": "retro",
        ])

        #expect(results.map(\.note.path) == ["Meetings/Retro 2026.md", "Archive/a.md", "b.md"])
        #expect(results[0].nameMatches && !results[1].nameMatches)
        #expect(results[0].lines.isEmpty)
    }

    @Test("tag: keeps the Notes with that Tag or one under it, in any case, with or without #")
    func tags() throws {
        let files = [
            "a.md": "---\ntags: [client/axo]\n---\nbudget",
            "b.md": "budget #client/bnp",
            "c.md": "budget #clients",
            "d.md": "budget",
        ]

        #expect(try search("budget tag:client", in: files).map(\.note.path) == ["a.md", "b.md"])
        #expect(try search("tag:#Client/Axo", in: files).map(\.note.path) == ["a.md"])
        #expect(try search("tag:client tag:axo", in: files).isEmpty)
        // Only a filter: every Note it keeps, without lines.
        #expect(try search("tag:clients", in: files).map { "\($0.note.path):\($0.lines.count)" } == ["c.md:0"])
    }

    @Test("path: keeps the Notes whose path holds it, in any case")
    func paths() throws {
        let files = [
            "Meetings/2026/a.md": "budget",
            "meetings-old/b.md": "budget",
            "Clients/Meetings.md": "budget",
            "c.md": "budget",
        ]

        #expect(try search("budget path:Meetings/", in: files).map(\.note.path) == ["Meetings/2026/a.md"])
        #expect(try search("path:meetings", in: files).map(\.note.path).sorted() == ["Clients/Meetings.md", "Meetings/2026/a.md", "meetings-old/b.md"])
        #expect(try search("budget path:Clients/ path:Meetings", in: files).map(\.note.path) == ["Clients/Meetings.md"])
    }

    @Test("a long line is cut around the first word found")
    func longLine() throws {
        let filler = String(repeating: "word ", count: 80)
        let results = try search("needle", in: ["a.md": "\(filler)needle \(filler)"])

        let line = try #require(results.first?.lines.first)
        #expect(line.text.contains("needle"))
        #expect(line.text.hasPrefix("…") && line.text.hasSuffix("…"))
        #expect(line.text.count <= 202)
    }

    @Test("a Note that cannot be read is left out")
    func unreadable() throws {
        let folder = try TemporaryVault(files: ["a.md": "budget"])
        try Data([0xFF, 0xFE, 0x00]).write(to: folder.url.appending(path: "bad.md"))
        let vault = Vault(root: folder.url)

        #expect(vault.search(SearchQuery("budget"), in: [Note(path: "a.md"), Note(path: "bad.md"), Note(path: "gone.md")]).results.map(\.note.path) == ["a.md"])
    }

    @Test("the query is read into words and filters")
    func query() {
        let query = SearchQuery("  Budget tag:#Client/Axo path:Meetings/  café ")

        #expect(query.words == ["budget", "cafe"])
        #expect(query.tags == ["client/axo"])
        #expect(query.paths == ["meetings/"])
        #expect(SearchQuery("tag: path:").isEmpty)
        #expect(!SearchQuery("tag:x").isEmpty)
    }

    @Test("quotes keep spaces in a phrase, a Tag or a path")
    func quotes() throws {
        let query = SearchQuery(#"tag:"x" path:"Folder 1/" "late order" budget"#)
        #expect(query.words == ["late order", "budget"])
        #expect(query.paths == ["folder 1/"])
        #expect(query.tags == ["x"])

        let files = ["Folder 1/a.md": "the late order, budget", "Folder 1/b.md": "order late budget", "Folder 2/c.md": "late order budget"]
        #expect(try search(#""late order" path:"Folder 1/""#, in: files).map(\.note.path) == ["Folder 1/a.md"])
    }

    @Test("past the limit, the Notes are counted and not listed, and each lists at most ten lines")
    func limits() throws {
        let folder = try TemporaryVault(files: Dictionary(uniqueKeysWithValues: (0..<12).map {
            ("n\(String(format: "%02d", $0)).md", String(repeating: "word\n", count: 15))
        }))
        let vault = Vault(root: folder.url)
        let notes = VaultIndex(tree: try vault.tree()).notes

        let found = vault.search(SearchQuery("word"), in: notes, limit: 5)

        #expect(found.total == 12)
        #expect(found.results.map(\.note.path) == ["n00.md", "n01.md", "n02.md", "n03.md", "n04.md"])
        #expect(found.results[0].lines.count == 10)
        #expect(found.results[0].moreLines == 5)
    }

    @Test("the words may be split between the name and the text; ß is ss")
    func acrossNameAndText() throws {
        let files = ["Retro.md": "it went well", "Straße.md": "", "b.md": "the strasse"]

        #expect(try search("retro well", in: files).map(\.note.path) == ["Retro.md"])
        #expect(try search("strasse", in: files).map(\.note.path) == ["Straße.md", "b.md"])
    }

    @Test("tag: with a final slash is the Tag; path: alone lists the Notes without reading them")
    func filters() throws {
        let files = ["a.md": "#client/axo", "Meetings/b.md": "x"]

        #expect(try search("tag:client/", in: files).map(\.note.path) == ["a.md"])
        let found = try search("path:meetings", in: files)
        #expect(found.map(\.note.path) == ["Meetings/b.md"] && found[0].lines.isEmpty)
    }

    @Test("a long line is cut around the word as written, accents and ß included")
    func excerptWithFolding() throws {
        let filler = String(repeating: "ß", count: 150)
        let results = try search("needle", in: ["a.md": "\(filler) needle \(filler)"])

        let line = try #require(results.first?.lines.first)
        #expect(line.text.contains("needle"))
    }

    @Test("a search stopped midway finds nothing more")
    func cancelled() throws {
        let folder = try TemporaryVault(files: ["a.md": "budget", "b.md": "budget"])
        let vault = Vault(root: folder.url)
        let notes = VaultIndex(tree: try vault.tree()).notes

        #expect(vault.search(SearchQuery("budget"), in: notes, isCancelled: { true }).results.isEmpty)
    }
}
