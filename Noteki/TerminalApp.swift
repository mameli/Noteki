import AppKit

/// The user's terminal, to open on a folder of the Vault: the first one installed of those that
/// open a folder handed to them, Terminal otherwise. Nothing is embedded in Noteki.
enum TerminalApp {
    /// By bundle identifier, the ones people install first: Ghostty, iTerm, Warp, WezTerm, kitty, Alacritty.
    private static let preferred = [
        "com.mitchellh.ghostty", "com.googlecode.iterm2", "dev.warp.Warp-Stable",
        "com.github.wez.wezterm", "net.kovidgoyal.kitty", "org.alacritty", "com.apple.Terminal",
    ]

    /// Where it is, looked up once.
    static let url: URL? = preferred.lazy.compactMap { NSWorkspace.shared.urlForApplication(withBundleIdentifier: $0) }.first

    /// Its name for the menus: "Open in Ghostty".
    static var name: String {
        url.map { FileManager.default.displayName(atPath: $0.path(percentEncoded: false)).replacingOccurrences(of: ".app", with: "") } ?? "Terminal"
    }

    /// A new window of it, in `folder`.
    static func open(_ folder: URL) {
        guard let url else { return }
        NSWorkspace.shared.open([folder], withApplicationAt: url, configuration: NSWorkspace.OpenConfiguration())
    }
}
