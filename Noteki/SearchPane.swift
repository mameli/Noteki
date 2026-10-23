import NotekiCore
import SwiftUI

/// Search: a field, and the Notes found with the lines that hold the words. A click on a Note
/// opens it, on a line opens it there.
struct SearchPane: View {
    @Bindable var model: AppModel
    @FocusState private var isTyping: Bool

    var body: some View {
        VStack(spacing: 0) {
            TextField("Search", text: $model.searchText, prompt: Text("Words, tag:, path:"))
                .textFieldStyle(.roundedBorder)
                .focused($isTyping)
                .padding(.horizontal, 10)
                .padding(.bottom, 6)
            Divider()
            results
                .frame(maxHeight: .infinity, alignment: .top)
        }
        .onAppear { takeKeyboard() }
        .onChange(of: model.wantsSearchField) { takeKeyboard() }
    }

    private func takeKeyboard() {
        guard model.wantsSearchField else { return }
        model.wantsSearchField = false
        // Once the field is in the window: set at once, it is ignored.
        Task { isTyping = true }
    }

    @ViewBuilder
    private var results: some View {
        let found = model.searchResults
        if found.results.isEmpty {
            if model.isSearching {
                ContentUnavailableView("Searching…", systemImage: "magnifyingglass")
            } else if !SearchQuery(model.searchText).isEmpty {
                ContentUnavailableView.search(text: model.searchText)
            } else {
                ContentUnavailableView(
                    "Search the Vault", systemImage: "magnifyingglass",
                    description: Text("Every word must be in the Note. tag:client and path:Meetings/ narrow it; quotes keep spaces.")
                )
            }
        } else {
            // Not a List: its table costs several MB, and a search may find hundreds of Notes.
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    Text(found.total == 1 ? "1 Note" : "\(found.total) Notes")
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                    ForEach(found.results, id: \.note) { result in
                        SearchResultView(result: result, open: model.open(_:atLine:))
                    }
                    if found.total > found.results.count {
                        let more = found.total - found.results.count
                        Text("\(more) more \(more == 1 ? "Note" : "Notes"): add a word to narrow the search.")
                            .font(.callout)
                            .foregroundStyle(.secondary)
                            .padding(12)
                    }
                }
            }
        }
    }
}

private struct SearchResultView: View {
    let result: SearchResult
    let open: (Note, Int?) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            row { open(result.note, nil) } label: {
                HStack(spacing: 6) {
                    Label(result.note.name, systemImage: "doc.text").fontWeight(.medium)
                    if !result.note.folder.isEmpty {
                        Text(result.note.folder)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .truncationMode(.head)
                    }
                }
            }
            ForEach(result.lines, id: \.line) { line in
                row { open(result.note, line.line) } label: {
                    Text(line.text)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                        .padding(.leading, 22)
                }
            }
            if result.moreLines > 0 {
                Text(result.moreLines == 1 ? "1 more line" : "\(result.moreLines) more lines")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .padding(.leading, 34)
                    .padding(.bottom, 2)
            }
        }
        .padding(.bottom, 6)
    }

    private func row(action: @escaping () -> Void, @ViewBuilder label: () -> some View) -> some View {
        Button(action: action) {
            label()
                .padding(.horizontal, 12)
                .padding(.vertical, 3)
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
    }
}
