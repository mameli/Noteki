import Foundation

// The messages between the app and the editor. Their TypeScript twins are in
// Editor/src/bridge.ts: a change here needs the same change there.

/// From the app to the editor.
enum EditorCommand: Encodable {
    case load(text: String)

    private enum CodingKeys: String, CodingKey {
        case type, text
    }

    func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .load(let text):
            try container.encode("load", forKey: .type)
            try container.encode(text, forKey: .text)
        }
    }
}

/// From the editor to the app.
enum EditorMessage: Decodable {
    case ready

    private enum CodingKeys: String, CodingKey {
        case type
    }

    init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "ready": self = .ready
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container, debugDescription: "Unknown editor message \(type)"
            )
        }
    }
}
