import CoreServices
import Foundation
import os

private let logger = Logger(subsystem: "app.noteki.noteki", category: "vault")

/// Watches the Vault's folder with FSEvents and reports the files that changed, on the main
/// thread. It only reports: the Vault decides what the paths mean (ticket 06).
@MainActor
final class VaultWatcher {
    struct Change {
        let urls: [URL]
        /// FSEvents lost or merged events: anything under the Vault may have changed.
        let mustRescan: Bool
    }

    private var stream: FSEventStreamRef?
    private let onChange: @MainActor (Change) -> Void

    /// Events closer than this arrive together: an editor's save is several events.
    private static let latency: CFTimeInterval = 0.2
    private static let rescanFlags = FSEventStreamEventFlags(
        kFSEventStreamEventFlagMustScanSubDirs | kFSEventStreamEventFlagUserDropped
            | kFSEventStreamEventFlagKernelDropped | kFSEventStreamEventFlagRootChanged
    )

    init(root: URL, onChange: @escaping @MainActor (Change) -> Void) {
        self.onChange = onChange
        // The stream holds the box, the box holds the watcher weakly: a callback already queued
        // when the watcher goes away finds nothing instead of freed memory.
        var context = FSEventStreamContext(
            version: 0, info: Unmanaged.passRetained(WeakWatcher(self)).toOpaque(),
            retain: nil, release: { info in
                guard let info else { return }
                Unmanaged<WeakWatcher>.fromOpaque(info).release()
            }, copyDescription: nil
        )
        let flags = UInt32(kFSEventStreamCreateFlagFileEvents | kFSEventStreamCreateFlagUseCFTypes)
        guard let stream = FSEventStreamCreate(
            nil, Self.callback, &context, [root.path(percentEncoded: false)] as CFArray,
            FSEventStreamEventId(kFSEventStreamEventIdSinceNow), Self.latency, flags
        ) else {
            logger.error("Cannot watch the Vault: changes from other apps will not show")
            return
        }
        self.stream = stream
        FSEventStreamSetDispatchQueue(stream, .main)
        FSEventStreamStart(stream)
    }

    isolated deinit {
        guard let stream else { return }
        FSEventStreamStop(stream)
        FSEventStreamInvalidate(stream)
        FSEventStreamRelease(stream)
    }

    private static let callback: FSEventStreamCallback = { _, info, count, paths, flags, _ in
        guard let info else { return }
        let urls = (unsafeBitCast(paths, to: NSArray.self) as? [String] ?? []).map { URL(filePath: $0) }
        let mustRescan = (0..<count).contains { flags[$0] & VaultWatcher.rescanFlags != 0 }
        // An address, not the pointer: it crosses into the main actor's closure as a plain value.
        let box = Int(bitPattern: info)
        // The stream runs on the main queue.
        MainActor.assumeIsolated {
            guard let pointer = UnsafeRawPointer(bitPattern: box) else { return }
            let watcher = Unmanaged<WeakWatcher>.fromOpaque(pointer).takeUnretainedValue().watcher
            watcher?.onChange(Change(urls: urls, mustRescan: mustRescan))
        }
    }
}

private final class WeakWatcher {
    weak var watcher: VaultWatcher?

    init(_ watcher: VaultWatcher) {
        self.watcher = watcher
    }
}
