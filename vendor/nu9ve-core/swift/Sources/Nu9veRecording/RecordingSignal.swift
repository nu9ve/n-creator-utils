import Darwin
import Foundation

/// nu9ve-recording-v1: tells every app of the suite on this Mac whether something is being
/// recorded, which session, and from which instant its clock counts. A producer writes
/// `<dir>/<app>.json` only when its state changes, then posts `app.nu9ve.recording.changed`
/// (`RecordingSignalPublisher`). The reader below (`problem`, `status`, `derive`, `isNewer`,
/// `sessionTime`) follows the contract's consumer rules, with the same cases as the crate
/// `nu9ve-recording`: `data/recording-examples-v1.json`.
public enum RecordingSignal {
    public static let contract = "nu9ve-recording/1"
    public static let notification = Notification.Name("app.nu9ve.recording.changed")
    public static let directoryVariable = "NU9VE_RECORDING_DIR"
    public static let maxBytes = 4 * 1024
    static let maxSafe: Int64 = (1 << 53) - 1

    public enum State: String, Sendable, CaseIterable { case idle, preparing, recording, finalizing }
    public enum Capture: String, Sendable, CaseIterable { case screen, systemAudio = "system_audio", microphone, camera }

    public struct Epoch: Equatable, Sendable {
        public static let hostClock = "coremedia-host-clock"
        public var clock: String, value: Int64, timescale: Int64
        public init(clock: String = Self.hostClock, value: Int64, timescale: Int64) {
            self.clock = clock; self.value = value; self.timescale = timescale
        }
    }

    /// One state file, already valid.
    public struct Record: Equatable, Sendable {
        public var app: String, appVersion: String
        public var pid: Int32, processStartUS: Int64, seq: Int64
        public var state: State
        public var sessionID: String?
        public var epoch: Epoch?
        public var epochWallClock: String?
        public var captures: [String]
        public var endedAt: String?
        public var writtenAt: String

        public func json() -> Data {
            func text(_ value: String?) -> Any { value ?? NSNull() }
            let epochValue: Any = epoch.map { ["clock": $0.clock, "value": NSNumber(value: $0.value), "timescale": NSNumber(value: $0.timescale)] } ?? NSNull()
            let object: [String: Any] = [
                "contract": RecordingSignal.contract, "app": app, "app_version": appVersion,
                "pid": NSNumber(value: pid), "process_start_us": NSNumber(value: processStartUS), "seq": NSNumber(value: seq),
                "state": state.rawValue, "session_id": text(sessionID), "epoch": epochValue,
                "epoch_wall_clock": text(epochWallClock), "captures": captures, "ended_at": text(endedAt), "written_at": writtenAt
            ]
            return (try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys, .withoutEscapingSlashes])) ?? Data()
        }
    }

    // MARK: §1 folder

    /// The folder, or nil when there is none (a relative `NU9VE_RECORDING_DIR` disables the signal).
    /// `home` is the user's home, or the app's QA root in QA, so a QA run never quiets the real apps (§1).
    public static func directory(environment: [String: String], home: URL?) -> URL? {
        if let override = environment[directoryVariable] {
            return override.hasPrefix("/") ? URL(fileURLWithPath: override, isDirectory: true) : nil
        }
        return home?.appendingPathComponent("Library/Application Support/nu9ve/recording", isDirectory: true)
    }

    // MARK: §2 and §4 reading

    private static let appPattern = try! NSRegularExpression(pattern: "^[a-z][a-z0-9-]{0,31}$")
    private static let uuidPattern = try! NSRegularExpression(pattern: "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
    private static let utcPattern = try! NSRegularExpression(pattern: "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\\.[0-9]{1,9})?Z$")
    private static let utcMillisPattern = try! NSRegularExpression(pattern: "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\\.[0-9]{3}Z$")
    private static let required = ["contract", "app", "app_version", "pid", "process_start_us", "seq", "state", "session_id",
                                   "epoch", "epoch_wall_clock", "captures", "ended_at", "written_at"]

    private static func matches(_ pattern: NSRegularExpression, _ value: Any?) -> Bool {
        guard let text = value as? String else { return false }
        return pattern.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)) != nil
    }

    public static func validApp(_ name: String) -> Bool { matches(appPattern, name) }

    /// §4: only `<app>.json` with a valid app name; temporaries and the rest are not listed.
    public static func isListed(_ fileName: String) -> Bool {
        fileName.hasSuffix(".json") && validApp(String(fileName.dropLast(5)))
    }

    /// A JSON integer, never a boolean or a float.
    private static func integer(_ value: Any?) -> Int64? {
        guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(),
              !CFNumberIsFloatType(number) else { return nil }
        return number.int64Value
    }

    /// nil when the file is valid, otherwise the first reason it is not, with the codes of the examples.
    public static func problem(fileName: String, data: Data) -> String? {
        if data.count > maxBytes { return "too_large" }
        if data.starts(with: [0xEF, 0xBB, 0xBF]) { return "bom" }
        guard String(data: data, encoding: .utf8) != nil,
              let parsed = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) else { return "not_json" }
        guard let doc = parsed as? [String: Any] else { return "not_object" }
        if let missing = required.first(where: { doc[$0] == nil }) { return "missing_" + missing }
        guard doc["contract"] as? String == contract else { return "bad_contract" }
        guard let app = doc["app"] as? String, app + ".json" == fileName, validApp(app) else { return "app_mismatch" }
        guard let version = doc["app_version"] as? String, !version.isEmpty else { return "bad_app_version" }
        guard let pid = integer(doc["pid"]), (1...Int64(UInt32.max)).contains(pid) else { return "bad_pid" }
        guard let start = integer(doc["process_start_us"]), (0...maxSafe).contains(start) else { return "bad_process_start_us" }
        guard let seq = integer(doc["seq"]), (1...maxSafe).contains(seq) else { return "bad_seq" }
        guard let state = (doc["state"] as? String).flatMap(State.init(rawValue:)) else { return "bad_state" }
        guard matches(utcPattern, doc["written_at"]) else { return "bad_written_at" }

        let session = doc["session_id"]
        if state == .idle {
            guard session is NSNull else { return "bad_session_id" }
        } else if !matches(uuidPattern, session) { return "bad_session_id" }

        let epoch = doc["epoch"], wall = doc["epoch_wall_clock"]
        if state == .recording || state == .finalizing {
            guard let fields = epoch as? [String: Any], fields["clock"] as? String == Epoch.hostClock,
                  let value = integer(fields["value"]), let scale = integer(fields["timescale"]),
                  (1...maxSafe).contains(scale), (-maxSafe...maxSafe).contains(value) else { return "bad_epoch" }
            guard matches(utcMillisPattern, wall) else { return "bad_epoch_wall_clock" }
        } else {
            guard epoch is NSNull else { return "bad_epoch" }
            guard wall is NSNull else { return "bad_epoch_wall_clock" }
        }

        guard let captures = doc["captures"] as? [Any], captures.allSatisfy({ ($0 as? String).flatMap(Capture.init(rawValue:)) != nil }),
              Set(captures.compactMap { $0 as? String }).count == captures.count,
              (state == .idle) == captures.isEmpty else { return "bad_captures" }

        let ended = doc["ended_at"]
        if state == .finalizing {
            guard matches(utcPattern, ended) else { return "bad_ended_at" }
        } else if !(ended is NSNull) { return "bad_ended_at" }
        return nil
    }

    /// The record, or nil when `problem` finds anything.
    public static func parse(fileName: String, data: Data) -> Record? {
        guard problem(fileName: fileName, data: data) == nil,
              let doc = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        let epoch = (doc["epoch"] as? [String: Any]).map {
            Epoch(clock: $0["clock"] as? String ?? "", value: integer($0["value"]) ?? 0, timescale: integer($0["timescale"]) ?? 1)
        }
        return Record(app: doc["app"] as? String ?? "", appVersion: doc["app_version"] as? String ?? "",
                      pid: Int32(truncatingIfNeeded: integer(doc["pid"]) ?? 0), processStartUS: integer(doc["process_start_us"]) ?? 0,
                      seq: integer(doc["seq"]) ?? 0, state: State(rawValue: doc["state"] as? String ?? "") ?? .idle,
                      sessionID: doc["session_id"] as? String, epoch: epoch, epochWallClock: doc["epoch_wall_clock"] as? String,
                      captures: doc["captures"] as? [String] ?? [], endedAt: doc["ended_at"] as? String,
                      writtenAt: doc["written_at"] as? String ?? "")
    }

    /// A process as the registry §4 sees it.
    public struct Process: Equatable, Sendable {
        public var ownUser: Bool, startUS: Int64
        public init(ownUser: Bool, startUS: Int64) { self.ownUser = ownUser; self.startUS = startUS }
    }

    /// The process table of this Mac: `proc_pidinfo(PROC_PIDTBSDINFO)`, start truncated to µs.
    public static func process(_ pid: Int32) -> Process? {
        var info = proc_bsdinfo()
        let size = Int32(MemoryLayout<proc_bsdinfo>.size)
        guard pid > 0, proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, size) == size else { return nil }
        return Process(ownUser: info.pbi_uid == getuid(),
                       startUS: Int64(info.pbi_start_tvsec) * 1_000_000 + Int64(info.pbi_start_tvusec))
    }

    public enum Status: String, Sendable { case invalid, stale, live }

    public static func isLive(_ record: Record, processes: (Int32) -> Process? = process) -> Bool {
        guard let found = processes(record.pid) else { return false }
        return found.ownUser && found.startUS == record.processStartUS
    }

    /// Invalid and stale both count as idle (§4).
    public static func status(fileName: String, data: Data, processes: (Int32) -> Process? = process) -> Status {
        guard let record = parse(fileName: fileName, data: data) else { return .invalid }
        return isLive(record, processes: processes) ? .live : .stale
    }

    public struct Session: Equatable, Sendable {
        public var app: String, sessionID: String, state: State, captures: [String], epoch: Epoch?
        public init(app: String, sessionID: String, state: State, captures: [String], epoch: Epoch?) {
            self.app = app; self.sessionID = sessionID; self.state = state; self.captures = captures; self.epoch = epoch
        }
    }

    public struct Derived: Equatable, Sendable {
        public var capturing: Bool, quiet: Bool, sessions: [Session]
        public static let nobody = Derived(capturing: false, quiet: false, sessions: [])
        /// The apps whose live file is in `recording`.
        public var recordingApps: [String] { sessions.filter { $0.state == .recording }.map(\.app) }
    }

    /// The §4 table: what a consumer derives from every file of the folder.
    public static func derive(files: [String: Data], processes: (Int32) -> Process? = process) -> Derived {
        let live = files.keys.sorted().compactMap { name -> Record? in
            guard isListed(name), let data = files[name], let record = parse(fileName: name, data: data),
                  isLive(record, processes: processes) else { return nil }
            return record
        }
        return Derived(capturing: live.contains { $0.state == .recording },
                       quiet: live.contains { $0.state == .preparing || $0.state == .recording },
                       sessions: live.filter { $0.state != .idle }.map {
                           Session(app: $0.app, sessionID: $0.sessionID ?? "", state: $0.state, captures: $0.captures, epoch: $0.epoch)
                       })
    }

    /// §4: act on a notification only if its file has a higher `seq` for the same process.
    public static func isNewer(seen: (pid: Int32, startUS: Int64, seq: Int64)?, file: (pid: Int32, startUS: Int64, seq: Int64)) -> Bool {
        guard let seen, seen.pid == file.pid, seen.startUS == file.startUS else { return true }
        return file.seq > seen.seq
    }

    /// §5: µs since the epoch, or nil before it (no event is written then).
    public static func sessionTime(epoch: Epoch, nowNS: Int64) -> Int64? {
        let product = Int64(epoch.value).multipliedFullWidth(by: 1_000_000_000)
        let (quotient, remainder) = epoch.timescale.dividingFullWidth(product)
        let epochNS = remainder < 0 ? quotient - 1 : quotient   // floor, like the contract
        guard nowNS >= epochNS else { return nil }
        return (nowNS - epochNS) / 1000
    }

    /// RFC 3339 in UTC with milliseconds, as `written_at` and `epoch_wall_clock` want it.
    public static func utc(_ date: Date) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        formatter.timeZone = TimeZone(identifier: "UTC")
        return formatter.string(from: date)
    }
}

