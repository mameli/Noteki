import NotekiCore
import Observation
import os

private let logger = Logger(subsystem: "app.noteki.noteki", category: "vault")

/// What a pane keeps about the Vault's Notes (their links, their Tags), read when the pane is
/// first shown, not at launch, and kept up to date from then on: holding it costs memory, and a
/// pane nobody opens should cost none (SPEC, Budget).
@MainActor
@Observable
final class ScannedNotes<Map: NoteScanned> {
    private(set) var map = Map()
    /// Reads not finished: the pane says so rather than showing nothing.
    var isReading: Bool { reads > 0 }
    /// Whether anything was read for this Vault yet: before that, an empty `map` is not an answer.
    private(set) var hasRead = false

    /// Called on the main thread after a read: the Notes read and the Notes dropped.
    @ObservationIgnored var didRead: (_ notes: [Note], _ gone: [Note]) -> Void = { _, _ in }

    @ObservationIgnored private let name: String
    /// The Notes read or about to be: Files' last listing, once started.
    @ObservationIgnored private var scanned: Set<Note> = []
    /// Reading one batch after the other keeps a later read from being overwritten by an earlier
    /// one that finished last.
    @ObservationIgnored private var queue: Task<Void, Never>?
    /// Counted from zero again for each Vault: a read begun in another one changes nothing here.
    @ObservationIgnored private var generation = 0
    private var reads = 0

    init(name: String) {
        self.name = name
    }

    /// Files was listed. Reads the Notes new to it and drops those gone: if the pane `isWanted`,
    /// or if it has been before.
    func listed(_ notes: [Note], in vault: Vault, isWanted: Bool) {
        guard isWanted || !scanned.isEmpty else { return }
        // A Vault without Notes has nothing to read.
        if notes.isEmpty, !hasRead { hasRead = true }
        let current = Set(notes)
        let added = notes.filter { !scanned.contains($0) }
        let gone = scanned.subtracting(current)
        scanned = current
        read(added, dropping: Array(gone), in: vault)
    }

    /// Notes changed on disk, saved by Noteki or not: read again, if the pane has been shown.
    /// Those new to Files are read once Files lists them.
    func changed(_ notes: [Note], in vault: Vault) {
        guard !scanned.isEmpty else { return }
        read(notes, in: vault)
    }

    /// Another Vault: nothing is kept, and what is still being read is dropped.
    func reset() {
        map = Map()
        scanned = []
        hasRead = false
        reads = 0
        generation += 1
    }

    private func read(_ notes: [Note], dropping gone: [Note] = [], in vault: Vault) {
        guard !notes.isEmpty || !gone.isEmpty else { return }
        let previous = queue
        let generation = generation
        reads += 1
        queue = Task {
            await previous?.value
            defer { if generation == self.generation { reads -= 1 } }
            guard generation == self.generation else { return }
            var start = map
            for note in gone { start.remove(note) }
            let started = ContinuousClock.now
            let read = start
            let updated = await Task.detached {
                var reading = read
                reading.rescan(notes, in: vault)
                return reading
            }.value
            guard generation == self.generation else { return }
            // Read by scripts/measure.
            logger.info("\(self.name, privacy: .public) read: \(notes.count) Notes in \((ContinuousClock.now - started).milliseconds) ms")
            map = updated
            hasRead = true
            didRead(notes, gone)
        }
    }
}
