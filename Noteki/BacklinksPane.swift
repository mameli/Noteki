import NotekiCore
import SwiftUI

/// The right sidebar: the Notes with a Wikilink to the Note in front, each with its line.
struct BacklinksPane: View {
    let model: AppModel

    var body: some View {
        ScrollView {
            // Not a List: its table costs several MB for the few rows a Note has.
            LazyVStack(alignment: .leading, spacing: 0) {
                Text("Backlinks")
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                if model.backlinks.isEmpty {
                    emptyState
                }
                ForEach(model.backlinks) { backlink in
                    Button { model.open(backlink) } label: {
                        BacklinkRow(backlink: backlink)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 4)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    @ViewBuilder
    private var emptyState: some View {
        if model.isReadingLinks {
            Label("Reading the links of your Notes…", systemImage: "hourglass")
        } else if model.isShowingNote, model.backlinksAreFound {
            Label("No other Note links to this one.", systemImage: "link")
        }
    }
}

private struct BacklinkRow: View {
    let backlink: Backlink

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 6) {
                Text(backlink.note.name).fontWeight(.medium)
                if !backlink.note.folder.isEmpty {
                    Text(backlink.note.folder)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.head)
                }
            }
            Text(backlink.text)
                .font(.callout)
                .foregroundStyle(.secondary)
                .lineLimit(3)
        }
    }
}

extension Backlink: @retroactive Identifiable {
    public var id: String { "\(note.path):\(line)" }
}
