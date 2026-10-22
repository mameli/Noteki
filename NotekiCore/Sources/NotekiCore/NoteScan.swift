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
        let chunk = 64
        DispatchQueue.concurrentPerform(iterations: (notes.count + chunk - 1) / chunk) { part in
            var read: [(Note, Parsed?)] = []
            for note in notes[(part * chunk)..<min(notes.count, (part + 1) * chunk)] {
                read.append((note, (try? vault.text(of: note)).map(parse)))
            }
            merged.withLock { map in for (note, parsed) in read { merge(&map, note, parsed) } }
        }
        map = merged.withLock { $0 }
        // Reading every Note leaves the allocator holding the pages it used for their text: given
        // back, the app's memory at rest is what is kept, not the scan's peak (SPEC, Budget).
        if notes.count > chunk { malloc_zone_pressure_relief(nil, 0) }
    }
}
