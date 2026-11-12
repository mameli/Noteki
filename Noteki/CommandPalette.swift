import AppKit
import SwiftUI

/// A command of the menus, as the Command palette lists it: read from the app's menu bar, so the
/// palette always has every command, with its shortcut, enabled and ticked as the menu has it.
struct PaletteCommand: Identifiable, Hashable {
    /// The path in the menus: `Format › Heading › Heading 2`.
    let id: String
    let title: String
    let menuPath: [String]
    let shortcut: String
    let isOn: Bool
    /// Where it is, to run it as a click in the menu would.
    let menu: NSMenu
    let index: Int

    static func == (a: PaletteCommand, b: PaletteCommand) -> Bool { a.id == b.id }
    func hash(into hasher: inout Hasher) { hasher.combine(id) }

    func run() {
        guard index < menu.items.count, menu.items[index].title == title else { return }
        menu.update()
        if menu.items[index].isEnabled { menu.performActionForItem(at: index) }
    }
}

@MainActor
enum CommandPaletteCatalog {
    /// Not commands: the system's, the windows' list, the palette itself.
    private static let left = ["Services", "Hide Noteki", "Hide Others", "Show All", "Start Dictation", "Emoji & Symbols", "AutoFill", "Command Palette…"]

    /// The enabled commands of the menu bar, in the order of the menus.
    static func commands() -> [PaletteCommand] {
        guard let bar = NSApp.mainMenu else { return [] }
        var found: [PaletteCommand] = []
        // The app's menu (About, Settings, Quit) last: the Note's commands first.
        for top in bar.items.dropFirst() + bar.items.prefix(1) {
            guard let menu = top.submenu else { continue }
            walk(menu, path: [menu.title.isEmpty ? top.title : menu.title], into: &found)
        }
        return found
    }

    private static func walk(_ menu: NSMenu, path: [String], into found: inout [PaletteCommand]) {
        // The menu's items enabled as they would be if it opened now.
        menu.update()
        for (index, item) in menu.items.enumerated() {
            guard !item.isSeparatorItem, !item.isHidden, item.view == nil, !item.title.isEmpty, !left.contains(item.title) else { continue }
            if let submenu = item.submenu {
                guard submenu !== NSApp.servicesMenu else { continue }
                walk(submenu, path: path + [item.title], into: &found)
                continue
            }
            // The Window menu's list of windows.
            if item.action == #selector(NSWindow.makeKeyAndOrderFront(_:)) { continue }
            guard item.isEnabled else { continue }
            found.append(PaletteCommand(
                id: (path + [item.title]).joined(separator: " › "),
                title: item.title,
                menuPath: path,
                shortcut: shortcut(of: item),
                isOn: item.state == .on,
                menu: menu,
                index: index
            ))
        }
    }

    /// `⇧⌘P`, as the menu draws it.
    static func shortcut(of item: NSMenuItem) -> String {
        let key = item.keyEquivalent
        guard !key.isEmpty else { return "" }
        var modifiers = item.keyEquivalentModifierMask
        // An uppercase letter means Shift.
        if key.count == 1, key.uppercased() == key, key.lowercased() != key { modifiers.insert(.shift) }
        var text = ""
        if modifiers.contains(.control) { text += "⌃" }
        if modifiers.contains(.option) { text += "⌥" }
        if modifiers.contains(.shift) { text += "⇧" }
        if modifiers.contains(.command) { text += "⌘" }
        let names: [Character: String] = [
            "\r": "↩", "\t": "⇥", "\u{1b}": "⎋", "\u{8}": "⌫", "\u{7f}": "⌫", " ": "Space",
            Character(UnicodeScalar(NSLeftArrowFunctionKey)!): "←", Character(UnicodeScalar(NSRightArrowFunctionKey)!): "→",
            Character(UnicodeScalar(NSUpArrowFunctionKey)!): "↑", Character(UnicodeScalar(NSDownArrowFunctionKey)!): "↓",
        ]
        if key.count == 1, let name = names[key.first!] { return text + name }
        return text + key.uppercased()
    }

    private static let recentKey = "recentCommands"

    /// The commands chosen last, the latest first.
    static var recent: [String] {
        UserDefaults.standard.stringArray(forKey: recentKey) ?? []
    }

    static func chose(_ command: PaletteCommand) {
        let list = [command.id] + recent.filter { $0 != command.id }
        UserDefaults.standard.set(Array(list.prefix(8)), forKey: recentKey)
    }

    /// The commands for what was typed, the best first: every word typed found in the command's
    /// name or its menus, ignoring case and accents. Nothing typed: the recent ones first.
    static func matches(_ typed: String, in commands: [PaletteCommand]) -> [PaletteCommand] {
        let fold = { (text: String) in text.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil) }
        let words = fold(typed).split(whereSeparator: \.isWhitespace).map(String.init)
        let recent = recent
        let rank = { (command: PaletteCommand) -> Int in
            let position = recent.firstIndex(of: command.id)
            return position.map { $0 - recent.count } ?? 0
        }
        guard !words.isEmpty else {
            return commands.enumerated().sorted { (rank($0.element), $0.offset) < (rank($1.element), $1.offset) }.map(\.element)
        }
        var scored: [(score: Int, order: Int, command: PaletteCommand)] = []
        for (order, command) in commands.enumerated() {
            let title = fold(command.title)
            let all = fold(command.id)
            guard words.allSatisfy({ all.contains($0) }) else { continue }
            let titleWords = title.split(whereSeparator: { !$0.isLetter && !$0.isNumber })
            var score = 3
            if title.hasPrefix(words[0]) { score = 0 }
            else if titleWords.contains(where: { $0.hasPrefix(words[0]) }) { score = 1 }
            else if words.allSatisfy({ title.contains($0) }) { score = 2 }
            scored.append((score * 100 + rank(command), order, command))
        }
        return scored.sorted { ($0.score, $0.order) < ($1.score, $1.order) }.map(\.command)
    }
}

/// The Command palette (⇧⌘P), as VS Code's: over the window, every command of the menus found by
/// part of its name, with its shortcut; Return runs it, Escape closes. It also teaches the
/// shortcuts: Help → Keyboard Shortcuts opens it.
struct CommandPalette: View {
    @Bindable var model: AppModel
    @State private var typed = ""
    @State private var commands: [PaletteCommand] = []
    @State private var rows: [PaletteCommand] = []
    @State private var selected = 0
    @FocusState private var isTyping: Bool

    private static let rowHeight: CGFloat = 30

    var body: some View {
        ZStack(alignment: .top) {
            Color.black.opacity(0.001)
                .onTapGesture { model.closePalette() }
            VStack(spacing: 0) {
                TextField("Type a command", text: $typed)
                    .textFieldStyle(.plain)
                    .interfaceFont(.title3)
                    .padding(12)
                    .focused($isTyping)
                    .onKeyPress(.downArrow) { move(by: 1) }
                    .onKeyPress(.upArrow) { move(by: -1) }
                    .onKeyPress(.escape) {
                        model.closePalette()
                        return .handled
                    }
                    .onSubmit { run(rows.indices.contains(selected) ? rows[selected] : nil) }
                Divider()
                if rows.isEmpty {
                    Text("No command").foregroundStyle(.secondary).padding(12)
                } else {
                    ScrollViewReader { scroller in
                        ScrollView {
                            VStack(spacing: 0) {
                                ForEach(Array(rows.enumerated()), id: \.element) { position, command in
                                    row(command, isSelected: position == selected)
                                        .id(position)
                                        .contentShape(.rect)
                                        .onTapGesture { run(command) }
                                }
                            }
                            .padding(6)
                        }
                        .frame(height: min(CGFloat(rows.count) * Self.rowHeight + 12, 400))
                        .onChange(of: selected) { _, position in scroller.scrollTo(position) }
                    }
                }
            }
            // Narrower in a narrow window (a Loose file's): never past its sides.
            .frame(maxWidth: 600)
            .background(Color.themePanel, in: .rect(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(.separator))
            .shadow(radius: 20, y: 8)
            .padding(.top, 60)
            .padding(.horizontal, 16)
        }
        .onAppear {
            commands = CommandPaletteCatalog.commands()
            find()
            Task { isTyping = true }
        }
        .onDisappear(perform: model.paletteDisappeared)
        .onChange(of: typed) { find() }
    }

    private func find() {
        rows = CommandPaletteCatalog.matches(typed, in: commands)
        selected = 0
    }

    private func row(_ command: PaletteCommand, isSelected: Bool) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "checkmark")
                .opacity(command.isOn ? 1 : 0)
                .imageScale(.small)
            Text(command.title)
            Text(command.menuPath.joined(separator: " › "))
                .foregroundStyle(isSelected ? AnyShapeStyle(.white.opacity(0.8)) : AnyShapeStyle(.secondary))
                .lineLimit(1)
            Spacer(minLength: 8)
            Text(command.shortcut)
                .monospaced()
                .foregroundStyle(isSelected ? AnyShapeStyle(.white.opacity(0.9)) : AnyShapeStyle(.secondary))
        }
        .padding(.horizontal, 8)
        .frame(height: Self.rowHeight)
        .foregroundStyle(isSelected ? AnyShapeStyle(.white) : AnyShapeStyle(.primary))
        .background(isSelected ? AnyShapeStyle(.tint) : AnyShapeStyle(.clear), in: .rect(cornerRadius: 5))
    }

    private func move(by step: Int) -> KeyPress.Result {
        guard !rows.isEmpty else { return .handled }
        selected = min(max(selected + step, 0), rows.count - 1)
        return .handled
    }

    private func run(_ command: PaletteCommand?) {
        guard let command else { return }
        CommandPaletteCatalog.chose(command)
        model.closePalette { command.run() }
    }
}
