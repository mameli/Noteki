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
            with: JSONEncoder().encode(EditorCommand.load(noteID: "Meetings/Retro.md", text: "# Retro\n"))
        ) as? [String: String]

        #expect(json == ["type": "load", "noteID": "Meetings/Retro.md", "text": "# Retro\n"])
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

    @Test("clearing the editor needs nothing else")
    func clear() throws {
        #expect(try EditorCommand.clear.json() == #"{"type":"clear"}"#)
    }

    @Test("the editor reports the text of the Note it was given")
    func changed() throws {
        let json = ##"{"type":"changed","noteID":"Meetings/Retro.md","revision":7,"text":"# Retro\nnew\n"}"##

        #expect(try EditorMessage(json: json) == .changed(noteID: "Meetings/Retro.md", revision: 7, text: "# Retro\nnew\n"))
        #expect(try EditorMessage(json: #"{"type":"ready"}"#) == .ready)
    }
}
