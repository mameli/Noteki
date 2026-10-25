import Foundation

// The messages between the app and the editor, as JSON with a `type`. Their TypeScript twins
// are in Editor/src/bridge.ts: a test checks both sides name the same commands and messages.

/// From the app to the editor.
public enum EditorCommand: Encodable, Sendable {
    /// Show this Note's text. The editor tags what it reports with `noteID`, so text typed in
    /// one Note can never be saved into another. `line`, counted from 1, is where the cursor goes
    /// and the view scrolls to: a Backlink opens its Note at the link. Without it, the start.
    case load(noteID: String, text: String, line: Int?)
    /// The same Note changed on disk: show its new text, the cursor kept at the same place in
    /// the text, outside the undo history. `revision` is the last report the app received: the
    /// editor replies "applied" only if it has nothing newer, otherwise it reports first and
    /// drops the reload, so no key typed meanwhile is lost.
    case reload(noteID: String, revision: Int, text: String)
    /// Report now any change not reported yet, before the app switches Note or quits.
    case flush
    /// Show nothing and take no typing: no Note is open, as after switching Vault.
    case clear
    /// From the Edit menu. The web view's own versions see only the lines on screen: CodeMirror
    /// draws no more than those (ADR 0001).
    case selectAll, undo, redo
    /// The Edit menu's Find items, searching the whole Note.
    case find(FindStep)
    /// Which of the Note's links lead nowhere: those `inners` are drawn as Unresolved links.
    case unresolvedLinks(noteID: String, inners: [String])
    /// The files the Note's Embeds and Markdown images show, by what the editor asked about: the
    /// editor draws the images and links the PDFs.
    case embeds(noteID: String, files: [String: EmbeddedFile])
    /// Source mode: the Markdown as written, every mark shown. Off, Live preview (SPEC).
    case sourceMode(on: Bool)
    /// Puts the cursor at the start of `line`, from 1, in the middle of the view: a search result
    /// in the Note already shown.
    case goToLine(noteID: String, line: Int)
    /// The answer to the editor's `completeLink` with the same `requestID`.
    case linkCompletions(requestID: Int, completions: LinkCompletions)

    public enum FindStep: String, Sendable {
        /// Show the find bar, with the selection as the text to find.
        case open
        case next, previous
    }

    public static let types = ["load", "reload", "flush", "clear", "selectAll", "undo", "redo", "find", "unresolvedLinks", "linkCompletions", "goToLine", "sourceMode", "embeds"]

    private enum CodingKeys: String, CodingKey {
        case type, noteID, revision, text, step, inners, requestID, completions, line, on, files
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .load(let noteID, let text, let line):
            try container.encode("load", forKey: .type)
            try container.encode(noteID, forKey: .noteID)
            try container.encode(text, forKey: .text)
            try container.encodeIfPresent(line, forKey: .line)
        case .reload(let noteID, let revision, let text):
            try container.encode("reload", forKey: .type)
            try container.encode(noteID, forKey: .noteID)
            try container.encode(revision, forKey: .revision)
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
        case .unresolvedLinks(let noteID, let inners):
            try container.encode("unresolvedLinks", forKey: .type)
            try container.encode(noteID, forKey: .noteID)
            try container.encode(inners, forKey: .inners)
        case .embeds(let noteID, let files):
            try container.encode("embeds", forKey: .type)
            try container.encode(noteID, forKey: .noteID)
            try container.encode(files, forKey: .files)
        case .sourceMode(let on):
            try container.encode("sourceMode", forKey: .type)
            try container.encode(on, forKey: .on)
        case .goToLine(let noteID, let line):
            try container.encode("goToLine", forKey: .type)
            try container.encode(noteID, forKey: .noteID)
            try container.encode(line, forKey: .line)
        case .linkCompletions(let requestID, let completions):
            try container.encode("linkCompletions", forKey: .type)
            try container.encode(requestID, forKey: .requestID)
            try container.encode(completions, forKey: .completions)
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
    /// The user clicked a Wikilink: what is between its brackets.
    case followLink(inner: String)
    /// The Note's links, what is between their brackets: which lead nowhere?
    case checkLinks(noteID: String, inners: [String])
    /// The user typed `[[`: which Notes can the link lead to? Answered by `linkCompletions`.
    case completeLink(requestID: Int)

    public static let types = ["ready", "changed", "followLink", "checkLinks", "completeLink"]

    private enum CodingKeys: String, CodingKey {
        case type, noteID, revision, text, inner, inners, requestID
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
        case "followLink":
            self = .followLink(inner: try container.decode(String.self, forKey: .inner))
        case "checkLinks":
            self = .checkLinks(
                noteID: try container.decode(String.self, forKey: .noteID),
                inners: try container.decode([String].self, forKey: .inners)
            )
        case "completeLink":
            self = .completeLink(requestID: try container.decode(Int.self, forKey: .requestID))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container, debugDescription: "Unknown editor message \(type)"
            )
        }
    }
}
