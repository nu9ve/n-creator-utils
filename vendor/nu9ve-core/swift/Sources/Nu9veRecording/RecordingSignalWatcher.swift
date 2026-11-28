import Foundation

/// The consumer side (nu9ve-recording-v1 §4) for an app that quiets its sounds, holds its notices or
/// writes a timeline: it reads the folder at start and on each `app.nu9ve.recording.changed`, and checks
/// the producer's life every 5 s only while someone records. At rest nothing runs.
@MainActor
public final class RecordingSignalWatcher {
    public private(set) var derived: RecordingSignal.Derived = .nobody
    /// Called on the main actor when `derived` changes.
    public var onChange: ((RecordingSignal.Derived) -> Void)?

    private let folder: URL?
    private let processes: (Int32) -> RecordingSignal.Process?
    private let center: DistributedNotificationCenter
    private var observer: (any NSObjectProtocol)?
    private var timer: Timer?
    /// §4: the last seq seen per app, to ignore an older notification.
    private var seen: [String: (pid: Int32, startUS: Int64, seq: Int64)] = [:]

    /// `folder` is `RecordingSignal.directory(environment:home:)`; nil means there is no signal.
    public init(folder: URL?, processes: @escaping (Int32) -> RecordingSignal.Process? = RecordingSignal.process,
                center: DistributedNotificationCenter = .default()) {
        self.folder = folder; self.processes = processes; self.center = center
    }

    public func start() {
        guard observer == nil, folder != nil else { return }
        observer = center.addObserver(forName: RecordingSignal.notification, object: nil, queue: .main) { [weak self] note in
            let app = note.object as? String
            MainActor.assumeIsolated { self?.changed(app: app) }
        }
        reload()
    }

    public func stop() {
        if let observer { center.removeObserver(observer) }
        observer = nil
        timer?.invalidate(); timer = nil
    }

    /// Reads the whole folder again: at start, on a notification and while checking a producer's life.
    public func reload() {
        guard let folder else { return }
        let files = RecordingFile.readFolder(folder)
        for (name, data) in files {
            if let record = RecordingSignal.parse(fileName: name, data: data) {
                seen[record.app] = (record.pid, record.processStartUS, record.seq)
            }
        }
        apply(RecordingSignal.derive(files: files, processes: processes))
    }

    /// A notification for one app: acted on only when its file is newer for the same process (§4).
    func changed(app: String?) {
        guard let folder, let app, RecordingSignal.validApp(app) else { reload(); return }
        let name = "\(app).json"
        if let data = try? RecordingFile.readCapped(folder.appendingPathComponent(name)),
           let record = RecordingSignal.parse(fileName: name, data: data),
           !RecordingSignal.isNewer(seen: seen[app], file: (record.pid, record.processStartUS, record.seq)) {
            return
        }
        reload()
    }

    private func apply(_ next: RecordingSignal.Derived) {
        // While someone records or prepares, a producer that died without telling stops counting within 5 s.
        if next.sessions.isEmpty {
            timer?.invalidate(); timer = nil
        } else if timer == nil {
            timer = Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { [weak self] _ in
                MainActor.assumeIsolated { self?.reload() }
            }
        }
        guard next != derived else { return }
        derived = next
        onChange?(next)
    }
}
