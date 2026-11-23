import Foundation
import Testing
@testable import Nu9veEngine

enum Fixture {
    /// The test engine, built next to the test bundle.
    static let url: URL = {
        Bundle(for: EndSignal.self).bundleURL
            .deletingLastPathComponent().appendingPathComponent("Nu9veEngineFixture")
    }()

    static func configuration(_ extra: [String] = []) -> EngineConfiguration {
        var configuration = EngineConfiguration(executable: url, arguments: ["serve"] + extra, expectedProtocol: "fixture-engine/1")
        configuration.backoff = .milliseconds(200)
        configuration.maximumBackoff = .milliseconds(400)
        configuration.shutdownGrace = .milliseconds(100)
        return configuration
    }

    static func client(_ extra: [String] = [], _ change: (inout EngineConfiguration) -> Void = { _ in }) -> EngineClient {
        var configuration = configuration(extra)
        change(&configuration)
        return EngineClient(configuration)
    }

    static func temporaryPath(_ name: String) -> String {
        FileManager.default.temporaryDirectory.appendingPathComponent("nu9ve-engine-\(UUID().uuidString)-\(name)").path
    }
}

/// Waits up to two seconds for `condition`.
func eventually(_ condition: () -> Bool) async -> Bool {
    for _ in 0..<200 {
        if condition() { return true }
        try? await Task.sleep(for: .milliseconds(10))
    }
    return condition()
}

func expectError(_ code: String, origin: EngineError.Origin = .client, dispatched: Bool? = nil,
                 _ body: () async throws -> Void, sourceLocation: SourceLocation = #_sourceLocation) async {
    do {
        try await body()
        Issue.record("expected \(code)", sourceLocation: sourceLocation)
    } catch {
        guard let error = error as? EngineError else { Issue.record("\(error)", sourceLocation: sourceLocation); return }
        #expect(error.code == code, "\(error)", sourceLocation: sourceLocation)
        #expect(error.origin == origin, sourceLocation: sourceLocation)
        if let dispatched { #expect(error.dispatched == dispatched, sourceLocation: sourceLocation) }
    }
}

@Test func handshakeChecksContractAndProtocol() async throws {
    let engine = Fixture.client()
    defer { engine.stop() }
    let info = try await engine.start()
    #expect(info.contract == "nu9ve-engine/1")
    #expect(info.protocol == "fixture-engine/1")
    #expect(info.answers("echo") && info.answers("lease") && !info.answers("nothing"))

    let otherContract = Fixture.client(["--contract", "nu9ve-engine/2"])
    await expectError(EngineError.Code.incompatibleEngine) { try await otherContract.start() }
    let otherProtocol = Fixture.client(["--protocol", "other-engine/1"])
    await expectError(EngineError.Code.incompatibleEngine) { try await otherProtocol.start() }
}

@Test func startupFailures() async {
    let slow = Fixture.client(["--info-delay-ms", "1500"]) { $0.startupTimeout = .milliseconds(200) }
    await expectError(EngineError.Code.startupTimeout) { try await slow.start() }
    let crashing = Fixture.client(["--crash-on-start"])
    await expectError(EngineError.Code.startupFailed) { try await crashing.start() }
    let missing = EngineClient(EngineConfiguration(executable: URL(fileURLWithPath: "/nonexistent/engine"), arguments: []))
    await expectError(EngineError.Code.engineMissing, dispatched: false) { try await missing.start() }
    let relative = EngineClient(EngineConfiguration(executable: URL(fileURLWithPath: "engine", relativeTo: nil), arguments: []))
    await expectError(EngineError.Code.engineMissing) { try await relative.start() }
}

@Test func concurrentCallsPairById() async throws {
    let engine = Fixture.client()
    defer { engine.stop() }
    try await withThrowingTaskGroup(of: Void.self) { group in
        for index in 0..<64 {
            group.addTask {
                let reply = try #require(try await engine.call("echo", ["n": .number(EngineNumber(index)), "text": "ñ\n"]).value)
                #expect(reply.value["n"]?.numberValue?.int == index)
                #expect(reply.value["text"] == "ñ\n")
            }
        }
        try await group.waitForAll()
    }
    // One process answered them all.
    let pid = try #require(try await engine.call("pid").value?.value.numberValue?.int)
    #expect(Int32(pid) == engine.processIdentifier)
}

@Test func notPortedIsAResult() async throws {
    let engine = Fixture.client()
    defer { engine.stop() }
    let answered = try await engine.call("not_ported")
    #expect(answered.notPorted == EngineNotPorted(reason: "method_unavailable", message: "This method is available in the oracle.",
                                                  answeredByClient: false))
    let unlisted = try await engine.call("not_in_any_list")
    #expect(unlisted.notPorted?.answeredByClient == true)
    #expect(unlisted.notPorted?.reason == "method_unavailable")
    await expectError("project_not_found", origin: .engine, dispatched: true) { _ = try await engine.call("fail") }
}

@Test func decodesData() async throws {
    struct Echo: Decodable, Sendable { let title: String }
    let engine = Fixture.client()
    defer { engine.stop() }
    let echo = try await engine.call("echo", ["title": "Día"], as: Echo.self)
    #expect(echo.value?.title == "Día")
}

@Test func deadlineRestartsTheEngine() async throws {
    let engine = Fixture.client()
    defer { engine.stop() }
    try await engine.start()
    let first = engine.processIdentifier
    await expectError(EngineError.Code.timeout, dispatched: true) {
        _ = try await engine.call("sleep", ["ms": 2000], timeout: .milliseconds(150))
    }
    // The first restart is immediate.
    #expect(try await engine.call("echo", ["after": true]).value?.value["after"] == true)
    #expect(engine.processIdentifier != first)
}

@Test func crashFailsPendingAndBacksOff() async throws {
    let engine = Fixture.client()
    defer { engine.stop() }
    try await engine.start()
    // The engine answers in order: the sleep, then the crash; the echo queued behind it is lost.
    let slow = Task { try await engine.call("sleep", ["ms": 200]) }
    try await Task.sleep(for: .milliseconds(30))
    let crash = Task { try await engine.call("crash") }
    try await Task.sleep(for: .milliseconds(30))
    let behind = Task { try await engine.call("echo") }
    #expect(try await slow.value.value?.value["slept"] == 200)
    await expectError(EngineError.Code.connectionLost, dispatched: true) { _ = try await crash.value }
    await expectError(EngineError.Code.connectionLost, dispatched: true) { _ = try await behind.value }
    // First crash: restart right away. It crashes again: now the engine waits out the backoff.
    await expectError(EngineError.Code.connectionLost) { _ = try await engine.call("crash") }
    #expect(await eventually { engine.info == nil })
    do {
        _ = try await engine.call("echo")
        Issue.record("expected engine_unavailable")
    } catch {
        #expect(error.code == EngineError.Code.engineUnavailable)
        #expect(error.dispatched == false)
        #expect(error.retryAfterSeconds == 1)
    }
    try await Task.sleep(for: .milliseconds(250))
    #expect(try await engine.call("echo", ["ok": true]).value?.value["ok"] == true)
}

@Test func boundedResponsesAndRequests() async throws {
    let engine = Fixture.client { $0.maxResponseBytes = 1000; $0.maxRequestBytes = 2000 }
    defer { engine.stop() }
    try await engine.start()
    let pid = engine.processIdentifier
    await expectError(EngineError.Code.responseTooLarge, dispatched: true) {
        _ = try await engine.call("big", ["bytes": 200_000])
    }
    // The line was discarded to its newline; the same process keeps answering.
    #expect(try await engine.call("big", ["bytes": 10]).value?.value == .string(String(repeating: "a", count: 10)))
    #expect(engine.processIdentifier == pid)
    await expectError(EngineError.Code.requestTooLarge, dispatched: false) {
        _ = try await engine.call("echo", ["pad": .string(String(repeating: "x", count: 3000))])
    }
}

@Test func protocolViolationsCloseTheConnection() async throws {
    let engine = Fixture.client()
    defer { engine.stop() }
    await expectError(EngineError.Code.invalidResponse, dispatched: true) { _ = try await engine.call("garbage") }
    // The violation was not a crash of the call itself, but the process is new.
    #expect(try await engine.call("echo").value != nil)
    await expectError(EngineError.Code.invalidResponse, dispatched: true) { _ = try await engine.call("wrong_id") }
}

@Test func cancellationLeavesLaterCallsPaired() async throws {
    let engine = Fixture.client()
    defer { engine.stop() }
    try await engine.start()
    let slow = Task { try await engine.call("sleep", ["ms": 300]) }
    try await Task.sleep(for: .milliseconds(50))
    slow.cancel()
    switch await slow.result {
    case .success: Issue.record("expected cancelled")
    case .failure(let error):
        let failure = error as? EngineError
        #expect(failure?.code == EngineError.Code.cancelled && failure?.dispatched == true)
    }
    // The late answer of the cancelled call is dropped; this one gets its own.
    #expect(try await engine.call("echo", ["mine": true]).value?.value["mine"] == true)
}

@Test func leaseHoldsUntilReleased() async throws {
    let engine = Fixture.client()
    defer { engine.stop() }
    let path = Fixture.temporaryPath("lease")
    let lease = try #require(try await engine.openLease("lease", ["path": .string(path)], release: "lease_release").value)
    #expect(lease.acquired.value["phase"] == "ready")
    #expect(FileManager.default.fileExists(atPath: path))
    #expect(lease.isHeld)
    // The shared process is not the lease's.
    try await engine.start()
    #expect(engine.processIdentifier != lease.processIdentifier)
    let released = try await lease.release()
    #expect(released.value["phase"] == "released")
    #expect(!FileManager.default.fileExists(atPath: path))
    #expect(!lease.isHeld)
    await expectError(EngineError.Code.leaseEnded) { _ = try await lease.release() }
}

@Test func leaseEndsWithEOF() async throws {
    let engine = Fixture.client()
    let path = Fixture.temporaryPath("eof")
    let lease = try #require(try await engine.openLease("lease", ["path": .string(path)], release: "lease_release").value)
    #expect(FileManager.default.fileExists(atPath: path))
    lease.close()
    #expect(await eventually { !FileManager.default.fileExists(atPath: path) })
}

@Test func leaseLossIsReported() async throws {
    let engine = Fixture.client()
    let ended = EndSignal()
    let path = Fixture.temporaryPath("lost")
    let lease = try #require(try await engine.openLease("lease", ["path": .string(path)], release: "lease_release",
                                                        onEnd: { ended.set($0.code) }).value)
    kill(lease.processIdentifier, SIGKILL)
    #expect(await eventually { ended.value == EngineError.Code.connectionLost })
    #expect(!lease.isHeld)
    try? FileManager.default.removeItem(atPath: path)
    let missing = try await engine.openLease("lease", ["path": .string(path)], release: "no_release_method")
    #expect(missing.notPorted?.reason == "method_unavailable")
}

final class EndSignal: @unchecked Sendable {
    private let lock = NSLock()
    private var code: String?
    func set(_ value: String) { lock.withLock { code = value } }
    var value: String? { lock.withLock { code } }
}
