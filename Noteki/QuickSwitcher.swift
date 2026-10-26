import NotekiCore
import SwiftUI

/// The Quick switcher: over the window, it finds a Note by part of its name and opens it, or
/// creates the Note when no Note has that name.
struct QuickSwitcher: View {
    @Bindable var model: AppModel
    @State private var typed = ""
    @State private var rows: [Row] = []
    @State private var selected = 0
    @FocusState private var isTyping: Bool

    /// The rows past these are not drawn: typing more finds the others.
    private static let shownMatches = 50
    private static let rowHeight: CGFloat = 28

    private enum Row: Hashable {
        case note(Note)
        case create(String)
    }

    /// Once per change of the typing, not at every redraw: arrow keys redraw too.
    private func findRows() {
        var found = model.switcherMatches(for: typed).prefix(Self.shownMatches).map(Row.note)
        if let name = model.switcherNewNote(for: typed) { found.append(.create(name)) }
        rows = found
        selected = 0
    }

    var body: some View {
        ZStack(alignment: .top) {
            // A click outside closes it, as Escape does.
            Color.black.opacity(0.001)
                .onTapGesture(perform: close)
            VStack(spacing: 0) {
                TextField("Find or create a Note", text: $typed)
                    .textFieldStyle(.plain)
                    .font(.title3)
                    .padding(12)
                    .focused($isTyping)
                    .onKeyPress(.downArrow) { move(by: 1) }
                    .onKeyPress(.upArrow) { move(by: -1) }
                    .onKeyPress(.escape) {
                        close()
                        return .handled
                    }
                    // ⌘Enter: in a new Tab.
                    .onKeyPress(.return, phases: .down) { press in
                        guard press.modifiers.contains(.command) else { return .ignored }
                        choose(rows.indices.contains(selected) ? rows[selected] : nil, inNewTab: true)
                        return .handled
                    }
                    .onSubmit { choose(rows.indices.contains(selected) ? rows[selected] : nil) }
                if !rows.isEmpty {
                    Divider()
                    ScrollViewReader { scroller in
                        ScrollView {
                            VStack(spacing: 0) {
                                ForEach(Array(rows.enumerated()), id: \.element) { position, row in
                                    rowView(row, isSelected: position == selected)
                                        .id(position)
                                        .contentShape(.rect)
                                        .onTapGesture { choose(row, inNewTab: NSEvent.modifierFlags.contains(.command)) }
                                }
                            }
                            .padding(6)
                        }
                        // As tall as its rows, up to a few of them: a ScrollView has no height of its own.
                        .frame(height: min(CGFloat(rows.count) * Self.rowHeight + 12, 360))
                        .onChange(of: selected) { _, position in scroller.scrollTo(position) }
                    }
                }
            }
            .frame(width: 560)
            .background(.regularMaterial, in: .rect(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(.separator))
            .shadow(radius: 20, y: 8)
            .padding(.top, 60)
        }
        .onAppear {
            findRows()
            // Once the field is in the window: from Files, setting it at once is ignored.
            Task { isTyping = true }
        }
        .onDisappear(perform: model.switcherDisappeared)
        .onChange(of: typed) { findRows() }
    }

    @ViewBuilder
    private func rowView(_ row: Row, isSelected: Bool) -> some View {
        HStack(spacing: 8) {
            switch row {
            case .note(let note):
                Text(note.name)
                Text(note.folder)
                    .foregroundStyle(isSelected ? AnyShapeStyle(.white.opacity(0.8)) : AnyShapeStyle(.secondary))
                    .lineLimit(1)
                    .truncationMode(.head)
            case .create(let name):
                Label("Create “\(name)”", systemImage: "plus")
            }
            Spacer(minLength: 0)
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

    private func choose(_ row: Row?, inNewTab: Bool = false) {
        switch row {
        case .note(let note): model.openFromSwitcher(note, inNewTab: inNewTab)
        case .create(let name): model.createFromSwitcher(name)
        case nil: return
        }
    }

    private func close() {
        model.closeSwitcher()
    }
}
