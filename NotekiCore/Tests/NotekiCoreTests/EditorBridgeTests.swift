import Foundation
import Testing
import NotekiCore

@Suite("Editor bridge")
struct EditorBridgeTests {
    @Test("the Swift and TypeScript sides know the same commands and the same messages")
    func contract() throws {
        let bridge = URL(filePath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appending(path: "Editor/src/bridge.ts")
        let source = try String(contentsOf: bridge, encoding: .utf8)
        let commands = try #require(source.firstRange(of: "export type EditorCommand"))
        let messages = try #require(source.firstRange(of: "export type EditorMessage"))
        func types(in part: Substring) -> Set<String> {
            Set(part.matches(of: /type: "(\w+)"/).map { String($0.1) })
        }

        #expect(types(in: source[commands.lowerBound..<messages.lowerBound]) == Set(EditorCommand.types))
        #expect(types(in: source[messages.lowerBound...]) == Set(EditorMessage.types))
    }

    @Test("a load command carries the Note and its text")
    func load() throws {
        let json = try JSONSerialization.jsonObject(
            with: JSONEncoder().encode(EditorCommand.load(noteID: "Meetings/Retro.md", text: "# Retro\n", line: nil))
        ) as? [String: String]

        #expect(json == ["type": "load", "noteID": "Meetings/Retro.md", "text": "# Retro\n"])
    }

    @Test("a load command can ask for a line, counted from 1")
    func loadAtLine() throws {
        let command = EditorCommand.load(noteID: "n1", text: "a\nb", line: 2)

        #expect(try command.json() == #"{"line":2,"noteID":"n1","text":"a\nb","type":"load"}"#)
    }

    @Test("editing commands from the app's menus need nothing else")
    func editingCommands() throws {
        #expect(try EditorCommand.selectAll.json() == #"{"type":"selectAll"}"#)
        #expect(try EditorCommand.undo.json() == #"{"type":"undo"}"#)
        #expect(try EditorCommand.redo.json() == #"{"type":"redo"}"#)
    }

    @Test("find says which step of the search to take")
    func find() throws {
        #expect(try EditorCommand.find(.open).json() == #"{"step":"open","type":"find"}"#)
        #expect(try EditorCommand.find(.next).json() == #"{"step":"next","type":"find"}"#)
        #expect(try EditorCommand.find(.previous).json() == #"{"step":"previous","type":"find"}"#)
    }

    @Test("a reload carries the Note, the last revision the app received and the new text")
    func reload() throws {
        let command = EditorCommand.reload(noteID: "Meetings/Retro.md", revision: 4, text: "# Retro\nTakku\n")

        #expect(try command.json() == ##"{"noteID":"Meetings\/Retro.md","revision":4,"text":"# Retro\nTakku\n","type":"reload"}"##)
    }

    @Test("the app tells the editor which of a Note's links are Unresolved")
    func unresolvedLinks() throws {
        let command = EditorCommand.unresolvedLinks(noteID: "n1", inners: ["Missing", "Other|alias"])

        #expect(try command.json() == #"{"inners":["Missing","Other|alias"],"noteID":"n1","type":"unresolvedLinks"}"#)
    }

    @Test("the editor asks for the Notes to complete a link, and the app answers that request")
    func linkCompletions() throws {
        let completions = LinkCompletions(
            recent: [LinkCompletion(name: "Retro", folder: "Meetings", link: "Retro")],
            others: [LinkCompletion(name: "Home", folder: "", link: "Home")]
        )

        #expect(try EditorMessage(json: #"{"type":"completeLink","requestID":3}"#) == .completeLink(requestID: 3))
        #expect(try EditorCommand.linkCompletions(requestID: 3, completions: completions).json() == #"{"completions":{"others":[{"folder":"","link":"Home","name":"Home"}],"recent":[{"folder":"Meetings","link":"Retro","name":"Retro"}]},"requestID":3,"type":"linkCompletions"}"#)
    }

    @Test("clearing the editor needs nothing else")
    func clear() throws {
        #expect(try EditorCommand.clear.json() == #"{"type":"clear"}"#)
    }

    @Test("the editor reports the text of the Note it was given")
    func changed() throws {
        let json = ##"{"type":"changed","noteID":"Meetings/Retro.md","revision":7,"text":"# Retro\nnew\n"}"##

        #expect(try EditorMessage(json: json) == .changed(noteID: "Meetings/Retro.md", revision: 7, text: "# Retro\nnew\n"))
        #expect(try EditorMessage(json: #"{"type":"ready"}"#) == .ready)
        #expect(try EditorMessage(json: #"{"type":"followLink","inner":"Retro|the retro"}"#) == .followLink(inner: "Retro|the retro"))
        #expect(try EditorMessage(json: #"{"type":"checkLinks","noteID":"n1","inners":["A","B|b"]}"#) == .checkLinks(noteID: "n1", inners: ["A", "B|b"]))
    }
}
