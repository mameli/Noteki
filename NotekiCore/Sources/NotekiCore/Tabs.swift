import Foundation

/// A Tab of a window: what it shows, a Note or an Attachment by its path, and what it showed
/// before, for back and forward (SPEC, Window).
public struct Tab: Codable, Equatable, Sendable, Identifiable {
    public let id: UUID
    /// The paths shown, the oldest first.
    public private(set) var history: [String]
    /// Which of them is shown.
    public private(set) var position: Int

    init(path: String) {
        id = UUID()
        history = [path]
        position = 0
    }

    public var path: String { history[position] }

    /// What the tab bar calls it: its Note's name, or its Attachment's file name.
    public var title: String {
        let name = path.split(separator: "/").last.map(String.init) ?? path
        return name.lowercased().hasSuffix(".md") ? String(name.dropLast(3)) : name
    }
    public var canGoBack: Bool { position > 0 }
    public var canGoForward: Bool { position < history.count - 1 }

    /// Shows `path`: what was forward is dropped, as in a browser. The same again is no step.
    mutating func show(_ path: String) {
        guard path != self.path else { return }
        history = Array(history[...position]) + [path]
        position += 1
    }

    mutating func back() {
        if canGoBack { position -= 1 }
    }

    mutating func forward() {
        if canGoForward { position += 1 }
    }

    mutating func renamed(from old: String, to new: String) {
        history = history.map { path in
            if path == old { return new }
            if path.hasPrefix(old + "/") { return new + path.dropFirst(old.count) }
            return path
        }
    }
}

/// The Tabs of a window and the one in front. Kept in the app's preferences, they come back at
/// launch.
public struct Tabs: Codable, Equatable, Sendable {
    public private(set) var all: [Tab] = []
    private var frontID: UUID?

    public init() {}

    public var front: Tab? { all.first { $0.id == frontID } }

    private var frontIndex: Int? { all.firstIndex { $0.id == frontID } }

    /// Shows `path` in the Tab in front, or in a new Tab right after it; the first path opens a Tab.
    public mutating func show(_ path: String, inNewTab: Bool = false) {
        guard let index = frontIndex, !inNewTab else {
            let tab = Tab(path: path)
            all.insert(tab, at: frontIndex.map { $0 + 1 } ?? all.count)
            frontID = tab.id
            return
        }
        all[index].show(path)
    }

    public mutating func select(_ id: Tab.ID) {
        if all.contains(where: { $0.id == id }) { frontID = id }
    }

    /// Closes a Tab; when it was in front, the one after it comes in front, or the one before
    /// when it was the last.
    public mutating func close(_ id: Tab.ID) {
        guard let index = all.firstIndex(where: { $0.id == id }) else { return }
        all.remove(at: index)
        guard id == frontID else { return }
        frontID = all.isEmpty ? nil : all[min(index, all.count - 1)].id
    }

    public mutating func selectNext() { step(by: 1) }
    public mutating func selectPrevious() { step(by: -1) }

    private mutating func step(by offset: Int) {
        guard let index = frontIndex else { return }
        frontID = all[(index + offset + all.count) % all.count].id
    }

    public mutating func back() {
        if let index = frontIndex { all[index].back() }
    }

    public mutating func forward() {
        if let index = frontIndex { all[index].forward() }
    }

    /// A Note, an Attachment or a folder moved: every Tab follows, its history too.
    public mutating func renamed(from old: String, to new: String) {
        for index in all.indices { all[index].renamed(from: old, to: new) }
    }

    /// The Tabs whose path is still `wanted`, as at launch, when a file may be gone.
    public func keeping(_ wanted: (String) -> Bool) -> Tabs {
        var kept = self
        kept.all = all.filter { wanted($0.path) }
        if kept.front == nil { kept.frontID = kept.all.last?.id }
        return kept
    }
}
