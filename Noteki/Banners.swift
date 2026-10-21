import NotekiCore
import SwiftUI

/// Shown while the user's edits and another app's change touch the same lines: nothing is
/// saved until the user chooses (ADR 0005).
struct ConflictBanner: View {
    let noteName: String
    let keepMine: () -> Void
    let takeTheirs: () -> Void
    let showDifferences: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Label(
                "Another app changed the same or adjacent lines of \(noteName). Your version is kept here, not saved.",
                systemImage: "exclamationmark.triangle.fill"
            )
            .foregroundStyle(.white)
            Spacer(minLength: 8)
            Button("Show Differences", action: showDifferences)
            Button("Take Theirs", action: takeTheirs)
            // No Return shortcut: pressing Return while writing must not erase the other change.
            Button("Keep Mine", action: keepMine)
                .buttonStyle(.borderedProminent)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.orange.opacity(0.95), in: .rect(cornerRadius: 8))
        .padding()
    }
}

/// The two versions of a Conflict side by side, the lines each one has and the other has not
/// highlighted.
struct DifferencesView: View {
    let conflict: OpenNote.Conflict
    let dismiss: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 0) {
                column("Your version", lines: marked(conflict.mine, against: conflict.theirs))
                Divider()
                column("The file now", lines: marked(conflict.theirs, against: conflict.mine))
            }
            Divider()
            HStack {
                Spacer()
                Button("Close", action: dismiss).keyboardShortcut(.cancelAction)
            }
            .padding()
        }
        .frame(minWidth: 760, minHeight: 480)
    }

    private func column(_ title: String, lines: [(text: Substring, isDifferent: Bool)]) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(title).font(.headline).padding(12)
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(lines.indices, id: \.self) { index in
                        Text(lines[index].text.isEmpty ? " " : String(lines[index].text))
                            .font(.system(.body, design: .monospaced))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 12)
                            .background(lines[index].isDifferent ? Color.orange.opacity(0.25) : .clear)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity)
    }

    /// Each line of `text`, marked when it is not among the lines `other` keeps in the same order.
    private func marked(_ text: String, against other: String) -> [(text: Substring, isDifferent: Bool)] {
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false)
        let others = other.split(separator: "\n", omittingEmptySubsequences: false)
        var added = Set<Int>()
        for case .insert(let offset, _, _) in lines.difference(from: others) { added.insert(offset) }
        return lines.enumerated().map { ($0.element, added.contains($0.offset)) }
    }
}

/// Shown while the open Note's file is gone, deleted or moved out of the Vault by another app:
/// its text is only on screen until the user saves it again.
struct DeletedBanner: View {
    let noteName: String
    let saveAgain: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Label(
                "\(noteName) was deleted, or moved out of the Vault, by another app. Its text is only here.",
                systemImage: "trash.fill"
            )
            .foregroundStyle(.white)
            Spacer(minLength: 8)
            Button("Save Again", action: saveAgain)
                .buttonStyle(.borderedProminent)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.gray.opacity(0.95), in: .rect(cornerRadius: 8))
        .padding()
    }
}
