import Foundation

// The messages between the app and the editor, as JSON with a `type`. Their TypeScript twins
// are in Editor/src/bridge.ts: a test checks both sides name the same commands and messages.

/// From the app to the editor.
public enum EditorCommand: Encodable, Sendable {
    /// Show this Note's text. The editor tags what it reports with `noteID`, so text typed in
    /// one Note can never be saved into another.
    case load(noteID: String, text: String)
    /// Report now any change not reported yet, before the app switches Note or quits.
    case flush
    /// Show nothing and take no typing: no Note is open, as after switching Vault.
    case clear
    /// From the Edit menu. The web view's own versions see only the lines on screen: CodeMirror
    /// draws no more than those (ADR 0001).
    case selectAll, undo, redo
    /// The Edit menu's Find items, searching the whole Note.
    case find(FindStep)

    public enum FindStep: String, Sendable {
        /// Show the find bar, with the selection as the text to find.
        case open
        case next, previous
    }

    public static let types = ["load", "flush", "clear", "selectAll", "undo", "redo", "find"]

    private enum CodingKeys: String, CodingKey {
        case type, noteID, text, step
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .load(let noteID, let text):
            try container.encode("load", forKey: .type)
            try container.encode(noteID, forKey: .noteID)
            try container.encode(text, forKey: .text)
        case .flush:
            try container.encode("flush", forKey: .type)
        case .clear:
            try container.encode("clear", forKey: .type)
        case .selectAll:
            try container.encode("selectAll", forKey: .type)
        case .undo:
            try container.encode("undo", forKey: .type)
        case .redo:
            try container.encode("redo", forKey: .type)
        case .find(let step):
            try container.encode("find", forKey: .type)
            try container.encode(step.rawValue, forKey: .step)
        }
    }

    /// The command as a JSON string, as the editor's `receive` takes it.
    public func json() throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        return String(decoding: try encoder.encode(self), as: UTF8.self)
    }
}

/// From the editor to the app.
public enum EditorMessage: Decodable, Equatable, Sendable {
    case ready
    /// The whole text of the Note after the user typed. `revision` grows with every report: a
    /// report that arrives after a later one is stale.
    case changed(noteID: String, revision: Int, text: String)

    public static let types = ["ready", "changed"]

    private enum CodingKeys: String, CodingKey {
        case type, noteID, revision, text
    }

    public init(json: String) throws {
        self = try JSONDecoder().decode(Self.self, from: Data(json.utf8))
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "ready":
            self = .ready
        case "changed":
            self = .changed(
                noteID: try container.decode(String.self, forKey: .noteID),
                revision: try container.decode(Int.self, forKey: .revision),
                text: try container.decode(String.self, forKey: .text)
            )
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container, debugDescription: "Unknown editor message \(type)"
            )
        }
    }
}
