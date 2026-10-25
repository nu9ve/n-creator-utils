import Foundation
import CoreServices

/// One event stream on the configured capture library; no timers or directory polling.
private final class WatcherContext: @unchecked Sendable {
    let callback: @Sendable ([String]) -> Void
    init(_ callback: @escaping @Sendable ([String]) -> Void) { self.callback = callback }
}
public final class CaptureWatcher: @unchecked Sendable {
    private var stream: FSEventStreamRef?
    public init(root: String, callback: @escaping @Sendable ([String]) -> Void) {
        let state = WatcherContext(callback)
        var context = FSEventStreamContext(version: 0, info: Unmanaged.passUnretained(state).toOpaque(), retain: { pointer in
            guard let pointer else { return nil }
            _ = Unmanaged<WatcherContext>.fromOpaque(pointer).retain(); return pointer
        }, release: { pointer in
            if let pointer { Unmanaged<WatcherContext>.fromOpaque(pointer).release() }
        }, copyDescription: nil)
        let handler: FSEventStreamCallback = { _, info, count, paths, flags, _ in
            guard let info else { return }
            let watcher = Unmanaged<WatcherContext>.fromOpaque(info).takeUnretainedValue()
            let values = unsafeBitCast(paths, to: NSArray.self) as? [String] ?? []
            let lost = (0..<count).contains { index in
                flags[index] & FSEventStreamEventFlags(kFSEventStreamEventFlagMustScanSubDirs | kFSEventStreamEventFlagUserDropped | kFSEventStreamEventFlagKernelDropped | kFSEventStreamEventFlagRootChanged) != 0
            }
            watcher.callback(lost ? [] : values)
        }
        stream = FSEventStreamCreate(nil, handler, &context, [root] as CFArray, FSEventStreamEventId(kFSEventStreamEventIdSinceNow), 0.8,
                                    FSEventStreamCreateFlags(kFSEventStreamCreateFlagUseCFTypes | kFSEventStreamCreateFlagFileEvents | kFSEventStreamCreateFlagWatchRoot))
        if let stream {
            FSEventStreamSetDispatchQueue(stream, DispatchQueue.global(qos: .utility))
            if !FSEventStreamStart(stream) { FSEventStreamInvalidate(stream); FSEventStreamRelease(stream); self.stream = nil }
        }
    }
    public var active: Bool { stream != nil }
    deinit {
        if let stream { FSEventStreamStop(stream); FSEventStreamInvalidate(stream); FSEventStreamRelease(stream) }
    }
}
