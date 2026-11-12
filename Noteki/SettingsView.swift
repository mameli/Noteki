import AppKit
import NotekiCore
import SwiftUI

/// Noteki → Settings… (⌘,): one page, as System Settings groups it. How the app looks, the
/// Note's text, writing, and the Vault: the folders it leaves out and where the Daily note is.
struct SettingsView: View {
    /// The Vault of the first window: its Excluded folders are shown.
    let model: AppModel

    var body: some View {
        Form {
            AppearanceSection()
            TextSection()
            WritingSection()
            VaultSection(model: model)
            UpdatesSection()
            SupportSection()
        }
        .formStyle(.grouped)
        .frame(minWidth: 460, idealWidth: 500, minHeight: 400, idealHeight: 540)
    }
}

/// The app's icon, name and version centred over the page, on the window's background: in the
/// first section's header, not a section of its own, which the grouped form would draw in a box.
private struct Header: View {
    var body: some View {
        VStack(spacing: 4) {
            Image(nsImage: NSApp.applicationIconImage)
                .resizable()
                .frame(width: 72, height: 72)
            Text("Noteki").font(.title2.weight(.semibold))
            if let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String {
                Text("Version \(version)").font(.body.weight(.regular)).foregroundStyle(.secondary)
            }
        }
        .foregroundStyle(.primary)
        .frame(maxWidth: .infinity)
        .padding(.bottom, 20)
    }
}

/// A binding to one of the editor's settings, written back as a whole.
@MainActor
private func editor<Value>(_ get: @escaping (EditorSettings) -> Value, set: @escaping (EditorSettings, Value) -> EditorSettings) -> Binding<Value> {
    Binding(
        get: { get(AppSettings.shared.editor) },
        set: { AppSettings.shared.editor = set(AppSettings.shared.editor, $0) }
    )
}

private struct AppearanceSection: View {
    @Bindable private var settings = AppSettings.shared

    var body: some View {
        Section {
            Picker("Theme", selection: editor(\.theme) { $0.with(theme: $1) }) {
                ForEach(Theme.allCases, id: \.self) { Text($0.name).tag($0) }
            }
            Picker("Light or dark", selection: $settings.appearance) {
                ForEach(Appearance.allCases) { Text($0.name).tag($0) }
            }
            .pickerStyle(.segmented)
            Picker("Interface size", selection: $settings.interfaceSize) {
                ForEach(InterfaceSize.allCases) { Text($0.name).tag($0) }
            }
            .pickerStyle(.segmented)
            .help("The sidebars, the Tabs and the panes; the Note's text has its own size.")
        } header: {
            VStack(alignment: .leading, spacing: 0) {
                Header()
                Text("Appearance")
            }
        }
    }
}

private struct TextSection: View {
    @Bindable private var settings = AppSettings.shared
    /// The families installed, read once.
    @State private var families = NSFontManager.shared.availableFontFamilies
    /// Those whose letters are all as wide, for code, read once.
    @State private var monoFamilies = NSFontManager.shared.availableFontNames(with: .fixedPitchFontMask).map { names in
        Array(Set(names.compactMap { NSFont(name: $0, size: 12)?.familyName })).sorted()
    } ?? []

    var body: some View {
        Section {
            // In the menu's own font: a thousand families each in its own would load them all.
            Picker("Font", selection: editor { $0.font ?? "" } set: { $0.with(font: .some($1)) }) {
                Text("System").tag("")
                Divider()
                ForEach(families, id: \.self) { Text($0).tag($0) }
            }
            Picker("Code font", selection: editor { $0.monoFont ?? "" } set: { $0.with(monoFont: .some($1)) }) {
                Text("System Monospaced").tag("")
                Divider()
                ForEach(monoFamilies, id: \.self) { Text($0).tag($0) }
            }
            LabeledContent("Size") {
                HStack {
                    Slider(value: editor(\.size) { $0.with(size: $1) }, in: EditorSettings.sizes, step: 1)
                        .labelsHidden()
                    Text("\(Int(settings.editor.size)) pt")
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                        .frame(width: 96, alignment: .trailing)
                }
            }
            Toggle("Limit the line length", isOn: editor(\.limitsLineLength) { $0.with(limitsLineLength: $1) })
            if settings.editor.limitsLineLength {
                LabeledContent("Line length") {
                    HStack {
                        Slider(value: editor(\.lineLength) { $0.with(lineLength: $1) }, in: EditorSettings.lineLengths, step: 5)
                            .labelsHidden()
                        Text("\(Int(settings.editor.lineLength)) characters")
                            .monospacedDigit()
                            .foregroundStyle(.secondary)
                            .frame(width: 96, alignment: .trailing)
                    }
                }
            }
        } header: {
            HStack {
                Text("Text")
                Spacer()
                // The Note's text as it was at first: theme, fonts, size, lines and spelling.
                Button("Restore Defaults") { settings.editor = .standard }
                    .buttonStyle(.link)
                    .font(.callout)
                    .disabled(settings.editor == .standard)
            }
        }
    }
}

private struct WritingSection: View {
    @Bindable private var settings = AppSettings.shared

    var body: some View {
        Section("Writing") {
            SpellingToggles(checks: "Check spelling while typing", corrects: "Correct spelling automatically")
            Toggle("Show the Format bar over the Note", isOn: $settings.showsFormatBar)
        }
    }
}

/// Sparkle's daily look for a new version, and the version found (UpdateChecker).
private struct UpdatesSection: View {
    @Bindable private var settings = AppSettings.shared
    private var updates = UpdateChecker.shared

    var body: some View {
        Section("Updates") {
            Toggle("Check for updates every day", isOn: $settings.checksForUpdates)
            if let release = updates.available {
                LabeledContent("Noteki \(release.version) is available") {
                    Button("Install…") { updates.check() }
                }
            } else {
                LabeledContent("Updates") {
                    Button("Check Now") { updates.check() }
                }
            }
        }
    }
}

/// An optional payment that supports Noteki's development. Nothing is locked: the app is the
/// same without it.
private struct SupportSection: View {
    @Environment(\.openURL) private var openURL

    var body: some View {
        Section("Support") {
            LabeledContent {
                Button("Support Noteki…") { openURL(HelpCommands.support) }
            } label: {
                Text("Noteki is free and open source. If it is useful to you, you can support its development. It works the same either way.")
                    .foregroundStyle(.secondary)
            }
        }
    }
}

/// The folders of the Vault the user leaves out, as the hidden ones are (not in Files, not
/// indexed, not searched), and where ⌘⇧D finds today's Daily note.
private struct VaultSection: View {
    let model: AppModel
    @State private var folders: [String] = []
    @State private var dailyFolder = AppSettings.shared.dailyNote.folder
    @State private var dailyFormat = AppSettings.shared.dailyNote.format

    var body: some View {
        Section {
            if let vault = model.vault {
                ForEach(folders, id: \.self) { folder in
                    HStack {
                        Label(folder, systemImage: "folder")
                        Spacer()
                        Button("Include “\(folder)” Again", systemImage: "minus.circle.fill") {
                            save(folders.filter { $0 != folder }, of: vault)
                        }
                        .labelStyle(.iconOnly)
                        .buttonStyle(.borderless)
                        .foregroundStyle(.secondary)
                    }
                }
                Button("Exclude a Folder…") { add(in: vault) }
            } else {
                Text("Open a Vault to choose the folders it leaves out.")
                    .foregroundStyle(.secondary)
            }
        } header: {
            Text("Excluded folders")
        } footer: {
            Text("Left out of Files, links, Tags and Search, as hidden folders are.")
                .foregroundStyle(.secondary)
        }
        // Another Vault chosen while Settings is open: its own folders.
        .onChange(of: model.vault?.root, initial: true) { _, root in
            folders = root.map { AppSettings.shared.excludedFolders(of: $0) } ?? []
        }

        Section {
            TextField("Folder", text: $dailyFolder, prompt: Text("The Vault's root"))
            TextField("Date format", text: $dailyFormat, prompt: Text(DailyNote.standard.format))
            LabeledContent("Today") {
                Text(DailyNote(folder: dailyFolder, format: dailyFormat).note(on: .now).path)
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
            }
        } header: {
            Text("Daily note (⌘⇧D)")
        } footer: {
            Text("Obsidian's format, moment.js's: YYYY, MM, DD, dddd, MMMM, WW…; text in [brackets] stays as written, a / makes folders. Names of days and months are English.")
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .onChange(of: dailyFolder) { saveDailyNote() }
        .onChange(of: dailyFormat) { saveDailyNote() }
    }

    private func saveDailyNote() {
        AppSettings.shared.dailyNote = DailyNote(folder: dailyFolder, format: dailyFormat)
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
        AppSettings.shared.setExcludedFolders(new, of: vault.root)
    }
}
