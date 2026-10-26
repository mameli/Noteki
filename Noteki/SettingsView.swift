import AppKit
import NotekiCore
import SwiftUI

/// Noteki → Settings… (⌘,): the editor's text, and the folders the Vault leaves out.
struct SettingsView: View {
    /// The Vault of the first window: its Excluded folders are shown.
    let model: AppModel

    var body: some View {
        TabView {
            EditorSettingsView()
                .tabItem { Label("Editor", systemImage: "textformat") }
            ExcludedFoldersView(model: model)
                .tabItem { Label("Excluded Folders", systemImage: "folder.badge.minus") }
        }
        .frame(width: 460)
    }
}

private struct EditorSettingsView: View {
    @Bindable private var settings = AppSettings.shared
    /// The families installed, read once.
    @State private var families = NSFontManager.shared.availableFontFamilies

    var body: some View {
        Form {
            // In the menu's own font: a thousand families each in its own would load them all.
            Picker("Font:", selection: Binding(
                get: { settings.editor.font ?? "" },
                set: { settings.editor = settings.editor.with(font: .some($0)) }
            )) {
                Text("System Font").tag("")
                Divider()
                ForEach(families, id: \.self) { Text($0).tag($0) }
            }
            Stepper(value: Binding(
                get: { settings.editor.size },
                set: { settings.editor = settings.editor.with(size: $0) }
            ), in: EditorSettings.sizes) {
                Text("Size: \(Int(settings.editor.size)) pt")
            }
            LabeledContent("Line width:") {
                HStack {
                    Slider(value: Binding(
                        get: { settings.editor.lineWidth },
                        set: { settings.editor = settings.editor.with(lineWidth: $0) }
                    ), in: EditorSettings.lineWidths)
                    Text("\(Int(settings.editor.lineWidth)) pt")
                        .monospacedDigit()
                        .frame(width: 60, alignment: .trailing)
                }
            }
            Button("Restore Defaults") { settings.editor = .standard }
                .disabled(settings.editor == .standard)
        }
        .padding(20)
    }
}

/// The folders of the Vault the user leaves out, as the hidden ones are: not in Files, not
/// indexed, not searched.
private struct ExcludedFoldersView: View {
    let model: AppModel
    @State private var folders: [String] = []
    @State private var selection: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let vault = model.vault {
                Text("Folders of “\(vault.root.lastPathComponent)” left out of Files, links, Tags and Search, as hidden folders are.")
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                List(folders, id: \.self, selection: $selection) { folder in
                    Label(folder, systemImage: "folder")
                }
                .frame(height: 160)
                HStack(spacing: 0) {
                    Button("Add Folder", systemImage: "plus") { add(in: vault) }
                    Button("Remove Folder", systemImage: "minus") {
                        if let selection { save(folders.filter { $0 != selection }, of: vault) }
                    }
                    .disabled(selection == nil)
                }
                .labelStyle(.iconOnly)
                .buttonStyle(.borderless)
            } else {
                Text("Open a Vault to choose the folders it leaves out.")
                    .foregroundStyle(.secondary)
            }
        }
        .padding(20)
        // Another Vault chosen while Settings is open: its own folders.
        .onChange(of: model.vault?.root, initial: true) { _, root in
            folders = root.map { AppSettings.shared.excludedFolders(of: $0) } ?? []
            selection = nil
        }
    }

    /// A folder chosen inside the Vault; one outside it, or the Vault itself, is refused.
    private func add(in vault: Vault) {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.directoryURL = vault.root
        panel.prompt = "Exclude"
        panel.message = "Choose a folder of the Vault to leave out."
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let place = vault.place(url)
        guard case .folder(let path) = place, !path.isEmpty else {
            let alert = NSAlert()
            // Hidden, code, or in a folder already left out.
            alert.messageText = place == .excluded
                ? "“\(url.lastPathComponent)” is already left out of the Vault."
                : "Choose a folder inside “\(vault.root.lastPathComponent)”."
            alert.runModal()
            return
        }
        save((folders + [path]).sorted { $0.localizedStandardCompare($1) == .orderedAscending }, of: vault)
    }

    private func save(_ new: [String], of vault: Vault) {
        folders = new
        selection = nil
        AppSettings.shared.setExcludedFolders(new, of: vault.root)
    }
}
