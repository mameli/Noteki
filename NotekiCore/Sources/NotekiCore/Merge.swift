/// Three-way merge of a Note, line by line (ADR 0005): the user's text and another app's, both
/// started from the same base. Lines neither side changed are fixed points; between two of them,
/// a part only one side changed takes that side, a part both changed the same way takes it once,
/// and a part both changed differently is a Conflict: the same lines, or adjacent ones with no
/// unchanged line between. Like diff3, repeated lines (blank ones) can align either way.
enum Merge {
    /// The merged text, nil on a Conflict.
    static func merge(base: String, mine: String, theirs: String) -> String? {
        let base = lines(base), mine = lines(mine), theirs = lines(theirs)
        let inMine = unchanged(from: base, to: mine)
        let inTheirs = unchanged(from: base, to: theirs)
        var merged: [Substring] = []
        // Where the part since the last fixed point starts, in each text.
        var (baseStart, mineStart, theirsStart) = (0, 0, 0)
        for (baseLine, mineLine) in inMine.sorted(by: { $0.key < $1.key }) {
            guard let theirsLine = inTheirs[baseLine] else { continue }
            let part = resolve(base[baseStart..<baseLine], mine[mineStart..<mineLine], theirs[theirsStart..<theirsLine])
            guard let part else { return nil }
            merged += part
            merged.append(base[baseLine])
            (baseStart, mineStart, theirsStart) = (baseLine + 1, mineLine + 1, theirsLine + 1)
        }
        guard let last = resolve(base[baseStart...], mine[mineStart...], theirs[theirsStart...]) else { return nil }
        merged += last
        return merged.joined(separator: "\n")
    }

    private static func resolve(
        _ base: ArraySlice<Substring>, _ mine: ArraySlice<Substring>, _ theirs: ArraySlice<Substring>
    ) -> ArraySlice<Substring>? {
        if mine.elementsEqual(base) { return theirs }
        if theirs.elementsEqual(base) || theirs.elementsEqual(mine) { return mine }
        return nil
    }

    /// The lines kept from `base` in `other`: base index → index in `other`.
    private static func unchanged(from base: [Substring], to other: [Substring]) -> [Int: Int] {
        let difference = other.difference(from: base)
        var removed = Set<Int>(), inserted = Set<Int>()
        for change in difference {
            switch change {
            case .remove(let offset, _, _): removed.insert(offset)
            case .insert(let offset, _, _): inserted.insert(offset)
            }
        }
        var kept: [Int: Int] = [:]
        var otherLine = 0
        for baseLine in base.indices where !removed.contains(baseLine) {
            while inserted.contains(otherLine) { otherLine += 1 }
            kept[baseLine] = otherLine
            otherLine += 1
        }
        return kept
    }

    /// Split on `\n` keeping the empty line after a final newline, so joining gives the text back.
    private static func lines(_ text: String) -> [Substring] {
        text.split(separator: "\n", omittingEmptySubsequences: false)
    }
}
