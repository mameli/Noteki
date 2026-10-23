import Darwin
import Dispatch
import Foundation
import Synchronization

/// What the Vault keeps about its Notes' text without keeping the text (ADR 0006): the places of
/// links, the Tags. It is read Note by Note, when a pane first needs it, and kept up to date.
public protocol NoteScanned: Sendable {
    init()
    /// Reads `notes` again, away from the main thread: what is kept for them is replaced, and a
    /// Note that cannot be read has nothing.
    mutating func rescan(_ notes: [Note], in vault: Vault)
    mutating func remove(_ note: Note)
}

enum NoteScan {
    /// Reads `notes` several at a time, because reading the files is what takes the time, and
    /// merges what `parse` makes of each text into `map`. A chunk is merged as soon as it is read,
    /// so no more than a few chunks' text is held at once; `merge` gets nil for a Note that cannot
    /// be read.
    static func read<Map: Sendable, Parsed: Sendable>(
        _ notes: [Note], in vault: Vault, into map: inout Map,
        parse: @Sendable (String) -> Parsed,
        merge: @Sendable (inout Map, Note, Parsed?) -> Void
    ) {
        let merged = Mutex(map)
        inChunks(of: notes.count) { positions in
            let read = positions.map { (notes[$0], (try? vault.text(of: notes[$0])).map(parse)) }
            merged.withLock { map in for (note, parsed) in read { merge(&map, note, parsed) } }
        }
        map = merged.withLock { $0 }
        givePagesBack(after: notes.count)
    }

    /// What `transform` makes of each Note's text, several Notes at a time, in the order of
    /// `notes`; nil, and a Note that cannot be read, leave nothing. Once `isCancelled`, the Notes not
    /// read yet are left out.
    static func map<Result: Sendable>(
        _ notes: [Note], in vault: Vault, isCancelled: @Sendable () -> Bool = { false },
        _ transform: @Sendable (Note, String) -> Result?
    ) -> [Result] {
        let results = Mutex([Result?](repeating: nil, count: notes.count))
        inChunks(of: notes.count) { positions in
            var found: [(Int, Result)] = []
            for position in positions {
                // Stopped: the Notes not read yet are not read.
                if isCancelled() { break }
                guard let text = try? vault.text(of: notes[position]), let result = transform(notes[position], text) else { continue }
                found.append((position, result))
            }
            results.withLock { all in for (position, result) in found { all[position] = result } }
        }
        givePagesBack(after: notes.count)
        return results.withLock { $0.compactMap(\.self) }
    }

    private static let chunk = 64

    /// Calls `body` with the positions of a chunk of the Notes, several chunks at a time.
    private static func inChunks(of count: Int, _ body: (Range<Int>) -> Void) {
        DispatchQueue.concurrentPerform(iterations: (count + chunk - 1) / chunk) { part in
            body((part * chunk)..<min(count, (part + 1) * chunk))
        }
    }

    /// Reading many Notes leaves the allocator holding the pages it used for their text: given back,
    /// the app's memory at rest is what is kept, not the scan's peak (SPEC, Budget).
    private static func givePagesBack(after count: Int) {
        if count > chunk { malloc_zone_pressure_relief(nil, 0) }
    }
}
