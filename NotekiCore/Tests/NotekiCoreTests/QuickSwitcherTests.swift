import Foundation
import Testing
import NotekiCore

@Suite("Quick switcher")
struct QuickSwitcherTests {
    private func index(_ files: [String]) throws -> (VaultIndex, TemporaryVault) {
        let folder = try TemporaryVault(files: Dictionary(uniqueKeysWithValues: files.map { ($0, "") }))
        return (VaultIndex(tree: try Vault(root: folder.url).tree()), folder)
    }

    @Test("with nothing typed, the Notes opened recently come first, then the others as Files lists them")
    func nothingTyped() throws {
        let (index, _) = try index(["Meetings/Retro.md", "A.md", "B.md", "Media/Photo.png"])

        let matches = index.switcherMatches(for: "", recent: [Note(path: "B.md"), Note(path: "Gone.md")], from: nil)

        #expect(matches.map(\.path) == ["B.md", "Meetings/Retro.md", "A.md"])
    }

    @Test("the name itself first, then names that start with the typing, a word that does, names that hold it, and its letters in order")
    func ranking() throws {
        let (index, _) = try index([
            "Rete Rossa.md", "Coretro.md", "Old retro.md", "Retrospective.md", "Retro.md", "Kickoff.md",
        ])

        let matches = index.switcherMatches(for: "retro", recent: [], from: nil)

        #expect(matches.map(\.name) == ["Retro", "Retrospective", "Old retro", "Coretro", "Rete Rossa"])
    }

    @Test("among names that match as well, the Note opened most recently comes first")
    func recentFirst() throws {
        let (index, _) = try index(["Retro 2026.md", "Retro 2025 long name.md", "Retro 2024.md"])

        let matches = index.switcherMatches(
            for: "retro", recent: [Note(path: "Retro 2025 long name.md"), Note(path: "Retro 2024.md")], from: nil
        )

        #expect(matches.map(\.name) == ["Retro 2025 long name", "Retro 2024", "Retro 2026"])
    }

    @Test("a better match beats a recent one")
    func matchBeforeRecency() throws {
        let (index, _) = try index(["Plan.md", "Planning old.md", "Spleen.md"])

        let matches = index.switcherMatches(for: "plan", recent: [Note(path: "Spleen.md"), Note(path: "Planning old.md")], from: nil)

        #expect(matches.map(\.name) == ["Plan", "Planning old"])
    }

    @Test("otherwise the shorter name first, then the order of Files")
    func shorterFirst() throws {
        let (index, _) = try index(["B/Kickoff meeting.md", "A/Kickoff call.md", "Kickoff 2.md"])

        let matches = index.switcherMatches(for: "kick", recent: [], from: nil)

        #expect(matches.map(\.path) == ["Kickoff 2.md", "A/Kickoff call.md", "B/Kickoff meeting.md"])
    }

    @Test("case, accents and spaces in the typing do not matter for letters in order")
    func folding() throws {
        let (index, _) = try index(["Café Milano.md", "Kickoff.md"])

        #expect(index.switcherMatches(for: "CAFE", recent: [], from: nil).map(\.name) == ["Café Milano"])
        #expect(index.switcherMatches(for: "cafè mil", recent: [], from: nil).map(\.name) == ["Café Milano"])
        #expect(index.switcherMatches(for: "cf ml", recent: [], from: nil).map(\.name) == ["Café Milano"])
        #expect(index.switcherMatches(for: "  ", recent: [Note(path: "Kickoff.md")], from: nil).first?.name == "Kickoff")
    }

    @Test("a path or a name with .md finds its Note first, as a Wikilink would")
    func typedLikeALink() throws {
        let (index, _) = try index(["Clients/Axo/Plan.md", "Archive/Plan.md", "Planning.md"])

        #expect(index.switcherMatches(for: "Clients/Axo/Plan", recent: [], from: nil).first == Note(path: "Clients/Axo/Plan.md"))
        #expect(index.switcherMatches(for: "axo/plan.md", recent: [], from: nil).map(\.path) == ["Clients/Axo/Plan.md"])
        // Not twice, and from the Note in front as a Wikilink: its folder's Plan first.
        #expect(index.switcherMatches(for: "plan", recent: [], from: Note(path: "Clients/Axo/Kickoff.md")).map(\.path)
            == ["Clients/Axo/Plan.md", "Archive/Plan.md", "Planning.md"])
    }

    @Test("a name no Note has is offered for creation, in the folder of the Note in front")
    func create() throws {
        let folder = try TemporaryVault(files: ["Clients/Axo/Plan.md": "", "Kickoff.md": ""])
        let vault = Vault(root: folder.url)
        let index = VaultIndex(tree: try vault.tree())
        let plan = Note(path: "Clients/Axo/Plan.md")

        #expect(index.switcherNewNote(for: "  Budget ", from: plan, in: vault) == "Budget")
        #expect(try vault.createNote(for: "Budget", from: plan) == Note(path: "Clients/Axo/Budget.md"))
    }

    @Test("no creation for a name a Note already has, in any case, nor one that cannot be a Note")
    func noCreate() throws {
        let folder = try TemporaryVault(files: ["Clients/Axo/Plan.md": "", "Kickoff.md": ""])
        let vault = Vault(root: folder.url)
        let index = VaultIndex(tree: try vault.tree())

        #expect(index.switcherNewNote(for: "kickoff", from: nil, in: vault) == nil)
        #expect(index.switcherNewNote(for: "Axo/Plan", from: nil, in: vault) == nil)
        #expect(index.switcherNewNote(for: "", from: nil, in: vault) == nil)
        // Not something a Wikilink can hold, nor a name the Vault refuses.
        for name in ["A#B", "A|B", "A^B", "[[A]]", "a:b", "photo.png", "../Outside", "node_modules/x"] {
            #expect(index.switcherNewNote(for: name, from: nil, in: vault) == nil, "\(name)")
        }
    }
}
