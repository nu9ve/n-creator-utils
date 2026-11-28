import Foundation
import Testing
@testable import Nu9veRecording

/// nu9ve-recording-v1, against `data/recording-examples-v1.json`: the same cases as the crate `nu9ve-recording`.
nonisolated(unsafe) private let examples: [String: Any] = {
    let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().appendingPathComponent("data/recording-examples-v1.json")
    let data = try! Data(contentsOf: url)
    return try! JSONSerialization.jsonObject(with: data) as! [String: Any]
}()

private func cases(_ key: String) -> [[String: Any]] { examples[key] as! [[String: Any]] }

private func table(_ value: Any?) -> (Int32) -> RecordingSignal.Process? {
    let rows = value as? [String: [String: Any]] ?? [:]
    return { pid in
        rows[String(pid)].map { RecordingSignal.Process(ownUser: $0["own_user"] as! Bool, startUS: ($0["start_us"] as! NSNumber).int64Value) }
    }
}

private func epoch(_ value: Any?) -> RecordingSignal.Epoch? {
    guard let fields = value as? [String: Any] else { return nil }
    return RecordingSignal.Epoch(clock: fields["clock"] as! String, value: (fields["value"] as! NSNumber).int64Value,
                                 timescale: (fields["timescale"] as! NSNumber).int64Value)
}

/// The producer of the examples, read from them.
private let exampleFile = cases("records").first { $0["name"] as? String == "recording" }!["file"] as! String

private func folder() throws -> URL {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent("nu9ve-signal-\(UUID().uuidString)/recording")
    return url
}

@Test func recordingFolderFollowsTheExamplesOnMacOS() {
    for example in cases("folders") where example["platform"] as? String == "macos" {
        let env = example["env"] as! [String: String]
        // §1, minor version of 2026-11-27: in QA the folder hangs from the QA root, which is the app's `home` there.
        let home = (example["qa_root"] as? String) ?? env["HOME"]
        let dir = RecordingSignal.directory(environment: env, home: home.map { URL(fileURLWithPath: $0, isDirectory: true) })
        #expect(dir?.path == example["dir"] as? String, "\(env)")
    }
}

@Test func aQARootKeepsTheSignalAwayFromTheRealApps() {
    let qa = URL(fileURLWithPath: "/tmp/qa-root", isDirectory: true)
    #expect(RecordingSignal.directory(environment: [:], home: qa)?.path
            == "/tmp/qa-root/Library/Application Support/nu9ve/recording")
    #expect(RecordingSignal.directory(environment: ["NU9VE_RECORDING_DIR": "qa/recording"], home: qa) == nil)
}

@Test func onlyAppStateFilesAreListed() {
    for example in cases("names") {
        #expect(RecordingSignal.isListed(example["file"] as! String) == example["listed"] as! Bool, "\(example["file"]!)")
    }
}

@Test func everyStateFileExampleGetsItsProblem() {
    let records = cases("records")
    #expect(records.count == 36)
    for example in records {
        var data = Data((example["text"] as! String).utf8)
        // JSONSerialization drops a leading U+FEFF inside a string, so the BOM example loses it on the way in.
        if example["problem"] as? String == "bom", !data.starts(with: [0xEF, 0xBB, 0xBF]) { data = Data([0xEF, 0xBB, 0xBF]) + data }
        let found = RecordingSignal.problem(fileName: example["file"] as! String, data: data)
        #expect(found == example["problem"] as? String, "\(example["name"]!)")
        #expect((RecordingSignal.parse(fileName: example["file"] as! String, data: data) != nil) == (found == nil))
    }
}

@Test func livenessFollowsThePidAndItsStartTime() {
    for example in cases("liveness") {
        let status = RecordingSignal.status(fileName: example["file"] as! String, data: Data((example["text"] as! String).utf8),
                                            processes: table(example["processes"]))
        #expect(status.rawValue == example["status"] as! String, "\(example["name"]!)")
    }
}

@Test func consumersDeriveCapturingQuietAndSessions() {
    for example in cases("derive") {
        let files = (example["files"] as! [String: String]).mapValues { Data($0.utf8) }
        let derived = RecordingSignal.derive(files: files, processes: table(example["processes"]))
        let expected = example["derived"] as! [String: Any]
        #expect(derived.capturing == expected["capturing"] as! Bool, "\(example["name"]!)")
        #expect(derived.quiet == expected["quiet"] as! Bool, "\(example["name"]!)")
        let sessions = (expected["sessions"] as! [[String: Any]]).map {
            RecordingSignal.Session(app: $0["app"] as! String, sessionID: $0["session_id"] as! String,
                                    state: RecordingSignal.State(rawValue: $0["state"] as! String)!,
                                    captures: $0["captures"] as! [String], epoch: epoch($0["epoch"]))
        }
        #expect(derived.sessions == sessions, "\(example["name"]!)")
    }
}

@Test func notificationsAreOrderedBySeqPerProcess() {
    func key(_ value: Any?) -> (pid: Int32, startUS: Int64, seq: Int64)? {
        guard let fields = value as? [String: Any] else { return nil }
        return ((fields["pid"] as! NSNumber).int32Value, (fields["process_start_us"] as! NSNumber).int64Value, (fields["seq"] as! NSNumber).int64Value)
    }
    for example in cases("order") {
        #expect(RecordingSignal.isNewer(seen: key(example["seen"]), file: key(example["file"])!) == example["act"] as! Bool, "\(example["name"]!)")
    }
}

@Test func sessionTimeCountsMicrosecondsFromTheEpoch() {
    for example in cases("session_time") {
        let t = RecordingSignal.sessionTime(epoch: epoch(example["epoch"])!, nowNS: (example["now_ns"] as! NSNumber).int64Value)
        #expect(t == (example["t_us"] as? NSNumber)?.int64Value, "\(example["name"]!)")
    }
}

@MainActor
@Test func producerWritesEveryStateBeforeItsNotification() throws {
    let dir = try folder()
    defer { try? FileManager.default.removeItem(at: dir.deletingLastPathComponent()) }
    let file = dir.appendingPathComponent("recorder.json")
    var posted: [(RecordingSignal.State, String?, Int64)] = []
    let me = RecordingSignal.Process(ownUser: true, startUS: 1_795_600_000_123_456)
    let signal = RecordingSignalPublisher(directory: dir, app: "recorder", appVersion: "0.6.0", pid: 5120, processStartUS: me.startUS,
                                          processes: { $0 == 5120 ? me : nil },
                                          post: { state, session, seq in
        // The file is the authority: it is already on disk, with the same seq, when the notification goes out.
        let written = RecordingSignal.parse(fileName: "recorder.json", data: try! Data(contentsOf: file))
        #expect(written?.state == state && written?.seq == seq && written?.sessionID == session)
        posted.append((state, session, seq))
    })
    let session = UUID()
    signal.preparing(sessionID: session, captures: [.microphone, .screen])
    let afterPreparing = RecordingSignal.derive(files: ["recorder.json": try Data(contentsOf: file)], processes: { $0 == 5120 ? me : nil })
    #expect(afterPreparing.quiet && !afterPreparing.capturing)
    signal.recording(sessionID: session, epoch: .init(value: 123_456_789_000, timescale: 1_000_000_000),
                     wallClock: Date(timeIntervalSince1970: 1_795_600_000.123), captures: [.screen, .systemAudio, .microphone])
    let recording = try #require(RecordingSignal.parse(fileName: "recorder.json", data: Data(contentsOf: file)))
    #expect(recording.sessionID == session.uuidString.lowercased())
    #expect(recording.captures == ["screen", "system_audio", "microphone"])
    #expect(recording.epochWallClock?.hasSuffix(".123Z") == true)
    signal.finalizing(endedAt: Date())
    let finalizing = try #require(RecordingSignal.parse(fileName: "recorder.json", data: Data(contentsOf: file)))
    #expect(finalizing.epoch == recording.epoch && finalizing.endedAt != nil)
    signal.idle()
    signal.idle()   // nothing changed: no write, no notification
    #expect(posted.map(\.0) == [.preparing, .recording, .finalizing, .idle])
    #expect(posted.map(\.2) == [1, 2, 3, 4])
    #expect(posted.last?.1 == nil)

    var info = stat()
    #expect(lstat(dir.path, &info) == 0 && info.st_mode & 0o777 == 0o700)
    #expect(lstat(file.path, &info) == 0 && info.st_mode & 0o777 == 0o600)
    let leftovers = try FileManager.default.contentsOfDirectory(atPath: dir.path)
    #expect(leftovers == ["recorder.json"])

    signal.withdraw()
    #expect(!FileManager.default.fileExists(atPath: file.path))
}

@MainActor
@Test func anotherLiveProcessIsNeverOverwrittenButAStaleOneIs() throws {
    let dir = try folder()
    defer { try? FileManager.default.removeItem(at: dir.deletingLastPathComponent()) }
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    let file = dir.appendingPathComponent(exampleFile)
    let app = String(exampleFile.dropLast(5))
    let other = cases("records").first { $0["name"] as? String == "recording" }!["text"] as! String   // pid 5120
    try Data(other.utf8).write(to: file)
    let otherAlive: (Int32) -> RecordingSignal.Process? = { $0 == 5120 ? .init(ownUser: true, startUS: 1_795_600_000_123_456) : nil }

    var posts = 0
    let blocked = RecordingSignalPublisher(directory: dir, app: app, appVersion: "0.6.0", pid: 6000, processStartUS: 1,
                                           processes: otherAlive, post: { _, _, _ in posts += 1 })
    blocked.preparing(sessionID: UUID(), captures: [.screen])
    blocked.withdraw()
    #expect(blocked.blocked && posts == 0)
    #expect(try String(contentsOf: file, encoding: .utf8) == other)

    let replacing = RecordingSignalPublisher(directory: dir, app: app, appVersion: "0.6.0", pid: 6000, processStartUS: 1,
                                             processes: { _ in nil }, post: { _, _, _ in posts += 1 })
    replacing.preparing(sessionID: UUID(), captures: [.screen])
    #expect(!replacing.blocked && posts == 1)
    #expect(RecordingSignal.parse(fileName: exampleFile, data: try Data(contentsOf: file))?.pid == 6000)
}

@MainActor
@Test func withdrawLeavesAFileThatIsNoLongerOurs() throws {
    let dir = try folder()
    defer { try? FileManager.default.removeItem(at: dir.deletingLastPathComponent()) }
    let file = dir.appendingPathComponent(exampleFile)
    let app = String(exampleFile.dropLast(5))
    let signal = RecordingSignalPublisher(directory: dir, app: app, appVersion: "0.6.0", pid: 6000, processStartUS: 1,
                                          processes: { _ in nil }, post: { _, _, _ in })
    signal.preparing(sessionID: UUID(), captures: [.screen])
    let other = cases("records").first { $0["name"] as? String == "recording" }!["text"] as! String
    try Data(other.utf8).write(to: file)
    signal.withdraw()
    #expect(FileManager.default.fileExists(atPath: file.path))
}

@MainActor
@Test func aFolderThatIsNotPrivateDisablesTheSignal() throws {
    let dir = try folder()
    defer { try? FileManager.default.removeItem(at: dir.deletingLastPathComponent()) }
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o755])
    var posts = 0
    let signal = RecordingSignalPublisher(directory: dir, app: "recorder", appVersion: "0.6.0", processes: { _ in nil }, post: { _, _, _ in posts += 1 })
    signal.preparing(sessionID: UUID(), captures: [.screen])
    #expect(posts == 0)
    #expect(!FileManager.default.fileExists(atPath: dir.appendingPathComponent("recorder.json").path))
}

/// A real process that dies while "recording" (kill -9) stops counting for every consumer.
@MainActor
@Test func aKilledProducerStopsCounting() throws {
    let dir = try folder()
    defer { try? FileManager.default.removeItem(at: dir.deletingLastPathComponent()) }
    let child = Process()
    child.executableURL = URL(fileURLWithPath: "/bin/sleep")
    child.arguments = ["30"]
    try child.run()
    let pid = child.processIdentifier
    let start = try #require(RecordingSignal.process(pid)).startUS
    let signal = RecordingSignalPublisher(directory: dir, app: "recorder", appVersion: "0.6.0", pid: pid, processStartUS: start, post: { _, _, _ in })
    let session = UUID()
    signal.preparing(sessionID: session, captures: [.screen])
    signal.recording(sessionID: session, epoch: .init(value: 1, timescale: 1), wallClock: Date(), captures: [.screen])
    let read = { ["recorder.json": try! Data(contentsOf: dir.appendingPathComponent("recorder.json"))] }
    #expect(RecordingSignal.derive(files: read()).capturing)

    kill(pid, SIGKILL)
    child.waitUntilExit()
    let after = RecordingSignal.derive(files: read())
    #expect(!after.capturing && !after.quiet && after.sessions.isEmpty)
}

/// The consumer: it reads at start, follows a producer, and stops counting one that died.
@MainActor
@Test func theWatcherFollowsAProducerAndForgetsADeadOne() throws {
    let dir = try folder()
    defer { try? FileManager.default.removeItem(at: dir.deletingLastPathComponent()) }
    let child = Process()
    child.executableURL = URL(fileURLWithPath: "/bin/sleep")
    child.arguments = ["30"]
    try child.run()
    let pid = child.processIdentifier
    let start = try #require(RecordingSignal.process(pid)).startUS
    var changes: [RecordingSignal.Derived] = []
    let watcher = RecordingSignalWatcher(folder: dir)
    watcher.onChange = { changes.append($0) }
    watcher.start()
    defer { watcher.stop() }
    #expect(watcher.derived == .nobody)

    let signal = RecordingSignalPublisher(directory: dir, app: "recorder", appVersion: "1.0", pid: pid, processStartUS: start,
                                          post: { _, _, _ in watcher.changed(app: "recorder") })
    let session = UUID()
    signal.preparing(sessionID: session, captures: [.screen])
    #expect(watcher.derived.quiet && !watcher.derived.capturing)
    signal.recording(sessionID: session, epoch: .init(value: 1, timescale: 1), wallClock: Date(), captures: [.screen])
    #expect(watcher.derived.capturing && watcher.derived.recordingApps == ["recorder"])

    // An older notification is ignored: same process, lower seq.
    let count = changes.count
    watcher.changed(app: "recorder")
    #expect(changes.count == count)

    kill(pid, SIGKILL)
    child.waitUntilExit()
    watcher.reload()
    #expect(watcher.derived == .nobody)
    #expect(changes.map(\.capturing) == [false, true, false].suffix(changes.count))
}
