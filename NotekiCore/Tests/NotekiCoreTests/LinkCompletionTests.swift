import Foundation
import Testing
import NotekiCore

@Suite("Wikilink completion")
struct LinkCompletionTests {
    private func index(_ files: [String]) throws -> (VaultIndex, TemporaryVault) {
        let folder = try TemporaryVault(files: Dictionary(uniqueKeysWithValues: files.map { ($0, "") }))
        return (VaultIndex(tree: try Vault(root: folder.url).tree()), folder)
    }

    @Test("every Note is offered by its name, with its folder, in the order Files lists them")
    func everyNote() throws {
        let (index, _) = try index(["Meetings/Retro.md", "Home.md", "Media/Photo.png"])

        let completions = index.linkCompletions(recent: [])

        #expect(completions.recent.isEmpty)
        #expect(completions.others == [
            LinkCompletion(name: "Retro", folder: "Meetings", link: "Retro"),
            LinkCompletion(name: "Home", folder: "", link: "Home"),
        ])
    }

    @Test("the Notes opened recently come first, the latest first, and are not offered twice")
    func recentFirst() throws {
        let (index, _) = try index(["A.md", "B.md", "C.md", "D.md"])

        let completions = index.linkCompletions(recent: [Note(path: "C.md"), Note(path: "A.md")])

        #expect(completions.recent.map(\.name) == ["C", "A"])
        #expect(completions.others.map(\.name) == ["B", "D"])
    }

    @Test("a recent Note no longer in the Vault is left out")
    func recentGone() throws {
        let (index, _) = try index(["A.md"])

        let completions = index.linkCompletions(recent: [Note(path: "Gone.md"), Note(path: "A.md")])

        #expect(completions.recent.map(\.name) == ["A"])
    }

    @Test("a name shared by several Notes is written as the shortest path that leads to only one")
    func ambiguous() throws {
        let (index, _) = try index([
            "Clients/Axo/Retro.md", "Clients/Bnp/Retro.md", "Archive/Clients/Axo/Retro.md", "Retro.md", "Home.md",
        ])

        let links = Dictionary(uniqueKeysWithValues: index.linkCompletions(recent: []).others
            .filter { $0.name == "Retro" }
            .map { ($0.folder, $0.link) })

        #expect(links["Clients/Bnp"] == "Bnp/Retro")
        #expect(links["Archive/Clients/Axo"] == "Archive/Clients/Axo/Retro")
        // `Clients/Axo/Retro` is also the end of the Archive's path: only the whole path is its own.
        #expect(links["Clients/Axo"] == "Clients/Axo/Retro")
        // At the root, the name alone is its exact path.
        #expect(links[""] == "Retro")
        #expect(links.count == 4)
    }

    @Test("what is written leads back to the Note chosen, from any Note, as Obsidian resolves it")
    func resolvesBack() throws {
        let files = [
            "Clients/Axo/Retro.md", "Clients/Axo/Old/Retro.md", "Data/Retro.md", "a/Retro.md", "Retro.md",
            "Zeta/Deep/Down/Retro.md", "Clients/Axo/Kickoff.md", "Home.md",
        ]
        let (index, _) = try index(files)

        for completion in index.linkCompletions(recent: []).others {
            let chosen = Note(path: (completion.folder.isEmpty ? "" : completion.folder + "/") + completion.name + ".md")
            for source in files.map(Note.init(path:)) {
                #expect(index.resolve(completion.link, from: source) == .note(chosen), "\(completion.link) from \(source.path)")
            }
        }
    }

    @Test("a Note whose name ends like an Attachment's is written so that it leads to the Note")
    func attachmentLookalike() throws {
        let (index, _) = try index(["Media/Report.pdf", "Media/Report.pdf.md", "Home.md"])

        let completion = try #require(index.linkCompletions(recent: []).others.first { $0.name == "Report.pdf" })

        #expect(index.resolve(completion.link, from: Note(path: "Home.md")) == .note(Note(path: "Media/Report.pdf.md")))
    }
}
