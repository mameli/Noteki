import Foundation

extension VaultIndex {
    /// The Notes the Quick switcher offers for what the user typed, the best first: the name
    /// itself, then names that start with the typing, a word that does, names that hold it, and
    /// names with its letters in order. Among those that match as well, the Notes in `recent`
    /// first (the latest first), then the shorter name, then the order of Files. With nothing
    /// typed: the recent Notes, then all the others as Files lists them. What a Wikilink from
    /// `source` would lead to comes first: a path, or a name with `.md`, finds its Note.
    public func switcherMatches(for typed: String, recent: [Note], from source: Note?) -> [Note] {
        let trimmed = typed.trimmingCharacters(in: .whitespaces)
        let query = fold(trimmed)
        let recency = Dictionary(recent.enumerated().map { ($1, $0) }, uniquingKeysWith: { first, _ in first })
        let letters = query.filter { !$0.isWhitespace }
        let ranked = notes.indices.compactMap { position -> (note: Note, key: [Int])? in
            let note = notes[position]
            let name = foldedNames[position]
            guard let quality = match(query, letters: letters, in: name) else { return nil }
            // With nothing typed every Note matches alike: only recency and Files' order count.
            let length = query.isEmpty ? 0 : name.count
            return (note, [quality, recency[note] ?? recent.count, length, position])
        }
        let matches = ranked.sorted { $0.key.lexicographicallyPrecedes($1.key) }.map(\.note)
        guard !trimmed.isEmpty, case .note(let linked)? = resolve(trimmed, from: source) else { return matches }
        return [linked] + matches.filter { $0 != linked }
    }

    /// The Note the Quick switcher offers to create for what the user typed, the name trimmed:
    /// nil when a Wikilink from `source` would already find a Note, or when it could not name
    /// one (a character a link cannot hold, an Attachment's name, outside the Vault…).
    public func switcherNewNote(for typed: String, from source: Note?, in vault: Vault) -> String? {
        let name = typed.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty, !name.contains(where: { "[]#^|".contains($0) }) else { return nil }
        guard resolve(name, from: source) == nil, (try? vault.newNotePath(for: name, from: source)) != nil else { return nil }
        return name
    }
}

/// How well `query` matches a folded name, 0 the best; nil if it does not.
private func match(_ query: String, letters: String, in name: String) -> Int? {
    if query.isEmpty || name == query { return 0 }
    if name.hasPrefix(query) { return 1 }
    let found = name.ranges(of: query)
    if !found.isEmpty {
        // A word of the name starts with the typing: after a space, a dash, a bracket…
        let startsWord = found.contains { range in
            let before = name[name.index(before: range.lowerBound)]
            return !before.isLetter && !before.isNumber
        }
        return startsWord ? 2 : 3
    }
    // Its letters in order, spaces in the typing ignored.
    var remaining = letters[...]
    for char in name where char == remaining.first {
        remaining.removeFirst()
        if remaining.isEmpty { return 4 }
    }
    return nil
}
