import AppKit
import Observation
import Sparkle
import SwiftUI
import os

private let logger = Logger(subsystem: "app.noteki.noteki", category: "updates")

/// Updates through Sparkle (ADR 0011, as Takku's and Dimmela's). At launch and once a day, when the
/// user leaves it on in Settings, it reads the appcast published with the latest GitHub release;
/// the request carries nothing about the user or the Vault. A newer version shows in Settings and
/// in the Noteki menu instead of a window popping up; choosing it opens Sparkle's window with what
/// is new and *Install Update*: Noteki downloads the disk image, checks its EdDSA signature and
/// Developer ID, quits as with ⌘Q (every Note saved, or the user asked) and opens again.
@MainActor
@Observable
final class UpdateChecker: NSObject {
    static let shared = UpdateChecker()

    struct Release: Equatable {
        let version: String
    }

    /// A release newer than the running app, shown in Settings and the menu.
    private(set) var available: Release?

    @ObservationIgnored private var controller: SPUStandardUpdaterController!

    override private init() {
        super.init()
        controller = SPUStandardUpdaterController(startingUpdater: false, updaterDelegate: self, userDriverDelegate: self)
    }

    func start() {
        controller.updater.automaticallyChecksForUpdates = AppSettings.shared.checksForUpdates
        controller.startUpdater()
    }

    /// From the switch in Settings.
    func setChecksAutomatically(_ isOn: Bool) {
        controller.updater.automaticallyChecksForUpdates = isOn
    }

    /// Sparkle's window: the new version with what changed and *Install Update*, or "You're up to date".
    func check() {
        controller.checkForUpdates(nil)
    }
}

extension UpdateChecker: SPUUpdaterDelegate {
    #if DEBUG
    /// `-testUpdateFeed URL` reads another appcast, to try an update with a local one.
    nonisolated func feedURLString(for updater: SPUUpdater) -> String? {
        UserDefaults.standard.string(forKey: "testUpdateFeed")
    }
    #endif

    nonisolated func updater(_ updater: SPUUpdater, didFindValidUpdate item: SUAppcastItem) {
        let version = item.displayVersionString
        logger.notice("Noteki \(version, privacy: .public) is available")
        MainActor.assumeIsolated { available = Release(version: version) }
    }

    nonisolated func updaterDidNotFindUpdate(_ updater: SPUUpdater) {
        MainActor.assumeIsolated { available = nil }
    }

    nonisolated func updater(_ updater: SPUUpdater, didAbortWithError error: any Error) {
        // Offline or rate-limited: nothing to show, tried again tomorrow.
        logger.notice("Update check failed: \(error.localizedDescription, privacy: .public)")
    }
}

extension UpdateChecker: SPUStandardUserDriverDelegate {
    /// A daily check that finds a version shows it in Settings and the menu: no window by itself.
    nonisolated var supportsGentleScheduledUpdateReminders: Bool { true }

    nonisolated func standardUserDriverShouldHandleShowingScheduledUpdate(
        _ update: SUAppcastItem, andInImmediateFocus immediateFocus: Bool
    ) -> Bool {
        false
    }

    /// Required with the gentle reminders: Settings and the menu already show the version.
    nonisolated func standardUserDriverWillHandleShowingUpdate(
        _ handleShowingUpdate: Bool, forUpdate update: SUAppcastItem, state: SPUUserUpdateState
    ) {}
}

/// *Check for Updates…* in the Noteki menu, after *About Noteki*: *Install Noteki …* once a newer
/// version is known.
struct UpdateCommands: Commands {
    var body: some Commands {
        CommandGroup(after: .appInfo) {
            UpdateMenuItem()
        }
    }
}

private struct UpdateMenuItem: View {
    private var updates = UpdateChecker.shared

    var body: some View {
        if let release = updates.available {
            Button("Install Noteki \(release.version)…") { updates.check() }
        } else {
            Button("Check for Updates…") { updates.check() }
        }
    }
}
