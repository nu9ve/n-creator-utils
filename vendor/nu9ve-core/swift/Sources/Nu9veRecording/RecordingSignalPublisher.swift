import Darwin
import Foundation
import os

/// The producer side (nu9ve-recording-v1 §2.1 and §3). It writes only on a state change, never on a
/// heartbeat. A failure is logged and never stops a recording: the signal is a courtesy to other apps.
@MainActor
public final class RecordingSignalPublisher {
    public typealias Post = @MainActor (_ state: RecordingSignal.State, _ sessionID: String?, _ seq: Int64) -> Void

    public let file: URL?
    private let app: String, appVersion: String
    private let pid: Int32, processStartUS: Int64
    private let processes: (Int32) -> RecordingSignal.Process?
    private let post: Post
    private let now: () -> Date
    private let log = Logger(subsystem: "app.nu9ve.recording", category: "publisher")
    private var seq: Int64 = 0
    /// Set when another live process of the same app owns the file: this one then never publishes.
    public private(set) var blocked = false
    public private(set) var last: RecordingSignal.Record?

    /// `directory` is `RecordingSignal.directory(environment:home:)`, with the app's QA root as `home`
    /// in QA. `app` is the producer's name in the registry (§2).
    public init(directory: URL?, app: String, appVersion: String,
                pid: Int32 = getpid(), processStartUS: Int64? = nil,
                processes: @escaping (Int32) -> RecordingSignal.Process? = RecordingSignal.process,
                post: Post? = nil, now: @escaping () -> Date = Date.init) {
        file = RecordingSignal.validApp(app) ? directory?.appendingPathComponent("\(app).json") : nil
        self.app = app; self.appVersion = appVersion.isEmpty ? "0" : appVersion; self.pid = pid
        self.processStartUS = processStartUS ?? RecordingSignal.process(pid)?.startUS ?? 0
        self.processes = processes; self.now = now
        self.post = post ?? { state, sessionID, seq in
            var info: [String: Any] = ["state": state.rawValue, "seq": NSNumber(value: seq)]
            if let sessionID { info["session_id"] = sessionID }
            DistributedNotificationCenter.default().postNotificationName(
                RecordingSignal.notification, object: app, userInfo: info, deliverImmediately: true)
        }
    }

    public func preparing(sessionID: UUID, captures: [RecordingSignal.Capture]) {
        publish(.preparing, sessionID: sessionID, captures: captures)
    }

    /// Call once the epoch is fixed and before the first sample is accepted.
    public func recording(sessionID: UUID, epoch: RecordingSignal.Epoch, wallClock: Date, captures: [RecordingSignal.Capture]) {
        publish(.recording, sessionID: sessionID, captures: captures, epoch: epoch, wallClock: wallClock)
    }

    /// Call when capture stops, before files are closed. Keeps the session and epoch of `recording`.
    public func finalizing(endedAt: Date) {
        guard let last, last.state == .recording, let session = last.sessionID, let epoch = last.epoch else { return }
        var record = last
        record.state = .finalizing; record.endedAt = RecordingSignal.utc(endedAt)
        write(record, sessionID: session, epoch: epoch)
    }

    /// The session is written, successfully or not. Nothing to do when nothing was published.
    public func idle() {
        guard let last, last.state != .idle else { return }
        publish(.idle, sessionID: nil, captures: [])
    }

    /// On an orderly quit: remove the file only if it is still this process's.
    public func withdraw() {
        guard let file, !blocked, last != nil else { return }
        if let data = try? RecordingFile.readCapped(file), let current = RecordingSignal.parse(fileName: file.lastPathComponent, data: data),
           current.pid != pid || current.processStartUS != processStartUS { return }
        if unlink(file.path) != 0, errno != ENOENT { log.error("Cannot remove the recording signal: \(String(cString: strerror(errno)), privacy: .public)") }
        last = nil
    }

    private func publish(_ state: RecordingSignal.State, sessionID: UUID?, captures: [RecordingSignal.Capture],
                         epoch: RecordingSignal.Epoch? = nil, wallClock: Date? = nil) {
        let order = RecordingSignal.Capture.allCases
        let record = RecordingSignal.Record(
            app: app, appVersion: appVersion, pid: pid, processStartUS: processStartUS, seq: 0, state: state,
            sessionID: sessionID?.uuidString.lowercased(), epoch: epoch, epochWallClock: wallClock.map(RecordingSignal.utc),
            captures: order.filter(captures.contains).map(\.rawValue), endedAt: nil, writtenAt: "")
        write(record, sessionID: record.sessionID, epoch: epoch)
    }

    private func write(_ draft: RecordingSignal.Record, sessionID: String?, epoch: RecordingSignal.Epoch?) {
        guard let file, !blocked else { return }
        do {
            try RecordingFile.prepareFolder(file.deletingLastPathComponent())
            if last == nil, try ownedByAnotherLiveProcess(file) {
                blocked = true
                log.error("Another live process of this app publishes the recording signal; this one does not.")
                return
            }
            var record = draft
            seq += 1
            record.seq = seq; record.writtenAt = RecordingSignal.utc(now())
            // Plain mode (nu9ve-fs-v1): fsync without flushing the drive cache while a capture writes.
            try RecordingFile.replace(file, with: record.json())
            last = record
            post(record.state, sessionID, record.seq)
        } catch {
            log.error("Cannot write the recording signal: \(error.localizedDescription, privacy: .public)")
        }
    }

    private func ownedByAnotherLiveProcess(_ file: URL) throws -> Bool {
        guard let data = try RecordingFile.readCapped(file),
              let current = RecordingSignal.parse(fileName: file.lastPathComponent, data: data) else { return false }
        let mine = current.pid == pid && current.processStartUS == processStartUS
        return !mine && RecordingSignal.isLive(current, processes: processes)
    }
}

/// The folder and the files of the signal, by the rules of the registry §5 and nu9ve-fs-v1.
enum RecordingFile {
    struct Failure: Error, LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }

    private static func failure(_ what: String) -> Failure { Failure(message: "\(what): \(String(cString: strerror(errno)))") }

    /// Folder 0700, owned by this user, never through a link.
    static func prepareFolder(_ folder: URL) throws {
        if !FileManager.default.fileExists(atPath: folder.path) {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true,
                                                    attributes: [.posixPermissions: 0o700])
        }
        var info = stat()
        guard lstat(folder.path, &info) == 0, (info.st_mode & S_IFMT) == S_IFDIR, info.st_uid == getuid(),
              info.st_mode & 0o077 == 0 else {
            throw Failure(message: "The recording signal folder is not private: \(folder.path)")
        }
    }

    /// A sibling `.<name>.<UUID>.tmp`, mode 0600, written whole, fsynced and renamed over `url`; then the
    /// folder is synced. Plain mode: no drive cache flush, for a small file written during a capture.
    static func replace(_ url: URL, with data: Data) throws {
        let folder = url.deletingLastPathComponent()
        let temporary = folder.appendingPathComponent(".\(url.lastPathComponent).\(UUID().uuidString).tmp")
        do {
            let fd = open(temporary.path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0o600)
            guard fd >= 0 else { throw failure("create the temporary of \(url.lastPathComponent)") }
            do {
                try data.withUnsafeBytes { buffer in
                    var offset = 0
                    while offset < buffer.count {
                        let written = Darwin.write(fd, buffer.baseAddress!.advanced(by: offset), buffer.count - offset)
                        if written < 0 {
                            if errno == EINTR { continue }
                            throw failure("write \(url.lastPathComponent)")
                        }
                        offset += written
                    }
                }
                while fsync(fd) != 0 { guard errno == EINTR else { throw failure("sync \(url.lastPathComponent)") } }
            } catch {
                close(fd)
                throw error
            }
            guard close(fd) == 0 else { throw failure("close \(url.lastPathComponent)") }
            guard rename(temporary.path, url.path) == 0 else { throw failure("replace \(url.lastPathComponent)") }
        } catch {
            try? FileManager.default.removeItem(at: temporary)
            throw error
        }
        let dir = open(folder.path, O_RDONLY)
        if dir >= 0 { _ = fsync(dir); close(dir) }
    }

    /// At most `maxBytes + 1`, never following a link. nil when there is no file; a link reads as empty
    /// (an invalid file).
    static func readCapped(_ file: URL) throws -> Data? {
        let fd = open(file.path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC)
        if fd < 0 {
            if errno == ENOENT { return nil }
            if errno == ELOOP { return Data() }
            throw failure("read \(file.lastPathComponent)")
        }
        defer { close(fd) }
        var buffer = [UInt8](repeating: 0, count: RecordingSignal.maxBytes + 1)
        var total = 0
        while total < buffer.count {
            let count = buffer.withUnsafeMutableBytes { read(fd, $0.baseAddress!.advanced(by: total), $0.count - total) }
            if count < 0 { if errno == EINTR { continue }; break }
            if count == 0 { break }
            total += count
        }
        return Data(buffer.prefix(total))
    }

    /// Every listed file of the folder, capped. A missing folder is an empty one.
    static func readFolder(_ folder: URL) -> [String: Data] {
        let names = (try? FileManager.default.contentsOfDirectory(atPath: folder.path)) ?? []
        var files: [String: Data] = [:]
        for name in names where RecordingSignal.isListed(name) {
            if let data = try? readCapped(folder.appendingPathComponent(name)) { files[name] = data }
        }
        return files
    }
}
