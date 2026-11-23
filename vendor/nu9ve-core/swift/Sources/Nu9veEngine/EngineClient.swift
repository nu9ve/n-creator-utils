import Foundation
import Synchronization

/// The client of a long-lived engine (nu9ve-engine-v1, NDJSON over stdio). It starts the
/// process on the first call, asks `engine_info` once and checks `contract` (and the app's
/// `protocol`), pairs concurrent calls by id, gives each a deadline, and starts a new process
/// after a crash, spacing restarts. `not_ported` is a result, not an error.
///
///     let engine = EngineClient(EngineConfiguration(
///         executable: URL(fileURLWithPath: "/abs/path/engine"), arguments: ["serve"],
///         expectedProtocol: "app-engine/1"))
///     switch try await engine.call("project_summary", ["project": .string(path)]) {
///     case .value(let reply): use(try reply.decode(Summary.self))
///     case .notPorted(let missing): askTheOracle(missing.reason)
///     }
public final class EngineClient: Sendable {
    public let configuration: EngineConfiguration

    private struct State {
        var connection: EngineConnection?
        var info: EngineInfo?
        var starting: Task<Ready, any Error>?
        var failures = 0
        var retryAt: ContinuousClock.Instant?
    }

    struct Ready: @unchecked Sendable {
        let connection: EngineConnection
        let info: EngineInfo
    }

    private let state = Mutex(State())

    public init(_ configuration: EngineConfiguration) {
        self.configuration = configuration
    }

    deinit {
        state.withLock { $0.connection }?.shutdown()
    }

    /// The handshake of the running process, if there is one.
    public var info: EngineInfo? { state.withLock { $0.info } }

    /// The engine's process id, while it runs (for diagnostics).
    public var processIdentifier: Int32? { state.withLock { $0.connection?.processIdentifier } }

    /// Starts the engine if needed and returns its handshake.
    @discardableResult
    public func start() async throws(EngineError) -> EngineInfo {
        try await engineErrors { try await self.ready().info }
    }

    /// Calls `method`. Throws `EngineError`: the method's own error (`origin == .engine`) or a
    /// transport one (`origin == .client`, with `dispatched`).
    public func call(_ method: String, _ params: EngineObject = [:], timeout: Duration? = nil,
                     idempotencyKey: String? = nil) async throws(EngineError) -> EngineOutcome<EngineReply> {
        try await engineErrors {
            let ready = try await self.ready()
            if self.configuration.checksMethods, method != "engine_info", !ready.info.answers(method) {
                return .notPorted(EngineNotPorted(reason: "method_unavailable",
                                                  message: "The engine does not list \(method).", answeredByClient: true))
            }
            let envelope = try await ready.connection.request(method, params, idempotencyKey: idempotencyKey,
                                                              timeout: timeout ?? self.configuration.requestTimeout)
            self.state.withLock { $0.failures = 0 }
            return try Self.outcome(envelope)
        }
    }

    /// Calls `method` and decodes `data` as `T`.
    public func call<T: Decodable & Sendable>(_ method: String, _ params: EngineObject = [:], as type: T.Type,
                                              timeout: Duration? = nil) async throws(EngineError) -> EngineOutcome<T> {
        let outcome = try await call(method, params, timeout: timeout)
        do {
            return try outcome.map { try $0.decode(T.self) }
        } catch {
            throw EngineError(client: EngineError.Code.invalidResponse, "Cannot decode \(method): \(error)", dispatched: true)
        }
    }

    /// Acquires a session lease on a process of its own (see `EngineLease`).
    public func openLease(_ method: String, _ params: EngineObject = [:], release: String,
                          timeout: Duration? = nil,
                          onEnd: (@Sendable (EngineError) -> Void)? = nil) async throws(EngineError) -> EngineOutcome<EngineLease> {
        try await EngineLease.open(configuration, method, params, release: release, timeout: timeout, onEnd: onEnd)
    }

    /// Closes stdin and lets the process end (§8). The next call starts a new one.
    public func stop() {
        let connection = state.withLock { state -> EngineConnection? in
            defer { state.connection = nil; state.info = nil }
            return state.connection
        }
        connection?.shutdown()
    }

    // MARK: Internals

    static func outcome(_ envelope: EngineEnvelope) throws -> EngineOutcome<EngineReply> {
        switch envelope.body {
        case .data(let value, let bytes):
            return .value(EngineReply(value: value, bytes: bytes))
        case .error(let error):
            let failure = EngineError(engine: error)
            if failure.code == "not_ported" {
                return .notPorted(EngineNotPorted(reason: failure.reason ?? "", message: failure.message, answeredByClient: false))
            }
            throw failure
        }
    }

    private func ready() async throws -> Ready {
        enum Step { case ready(Ready), wait(Task<Ready, any Error>) }
        let step = try state.withLock { state throws(EngineError) -> Step in
            if let connection = state.connection, let info = state.info, !connection.isClosed {
                return .ready(Ready(connection: connection, info: info))
            }
            if let starting = state.starting { return .wait(starting) }
            if let retryAt = state.retryAt, retryAt > .now {
                let seconds = Int((retryAt - .now).components.seconds) + 1
                throw EngineError(client: EngineError.Code.engineUnavailable,
                                  "The engine failed; next start in \(seconds) s.",
                                  details: ["retry_after_seconds": .number(EngineNumber(seconds))])
            }
            let task = Task { try await self.launch() }
            state.starting = task
            return .wait(task)
        }
        switch step {
        case .ready(let ready): return ready
        case .wait(let task): return try await task.value
        }
    }

    private func launch() async throws -> Ready {
        do {
            let connection = try EngineConnection.launch(configuration) { [weak self] connection, _, expected in
                self?.closed(connection, expected: expected)
            }
            let info: EngineInfo
            do {
                info = try await Self.handshake(connection, configuration)
            } catch {
                connection.close(error as? EngineError ?? EngineError(client: EngineError.Code.startupFailed, "\(error)"),
                                 expected: true)
                throw error
            }
            state.withLock { state in
                state.connection = connection
                state.info = info
                state.starting = nil
            }
            return Ready(connection: connection, info: info)
        } catch {
            state.withLock { state in
                state.starting = nil
                failed(&state)
            }
            throw error
        }
    }

    private func closed(_ connection: EngineConnection, expected: Bool) {
        state.withLock { state in
            guard state.connection === connection else { return }
            state.connection = nil
            state.info = nil
            if !expected { failed(&state) }
        }
    }

    /// The first restart is immediate; then `backoff`, doubling up to `maximumBackoff`.
    private func failed(_ state: inout State) {
        if state.failures > 0 {
            let steps = min(state.failures - 1, 30)
            let delay = min(configuration.maximumBackoff, configuration.backoff * Double(1 << steps))
            state.retryAt = .now + delay
        }
        state.failures += 1
    }

    /// `engine_info` with the startup deadline, checked against §4 and the app's protocol.
    static func handshake(_ connection: EngineConnection, _ configuration: EngineConfiguration) async throws -> EngineInfo {
        let envelope: EngineEnvelope
        do {
            envelope = try await connection.request("engine_info", [:], timeout: configuration.startupTimeout)
        } catch let error as EngineError {
            let code = error.code == EngineError.Code.timeout ? EngineError.Code.startupTimeout : EngineError.Code.startupFailed
            throw EngineError(client: code, "Handshake failed: \(error.message)")
        }
        guard case .data(let data, _) = envelope.body else {
            throw EngineError(client: EngineError.Code.incompatibleEngine, "The engine answered engine_info with an error.")
        }
        let info: EngineInfo
        switch EngineInfo.parse(data) {
        case .success(let parsed): info = parsed
        case .failure(let problem):
            throw EngineError(client: EngineError.Code.incompatibleEngine, "Unusable engine_info (\(problem.rawValue)).")
        }
        if let expected = configuration.expectedProtocol, info.protocol != expected {
            throw EngineError(client: EngineError.Code.incompatibleEngine, "The engine speaks \(info.protocol), not \(expected).")
        }
        return info
    }
}

/// Runs `body` and gives any failure as `EngineError`.
func engineErrors<T: Sendable>(_ body: () async throws -> T) async throws(EngineError) -> T {
    do {
        return try await body()
    } catch let error as EngineError {
        throw error
    } catch is CancellationError {
        throw EngineError(client: EngineError.Code.cancelled, "The call was cancelled.")
    } catch {
        throw EngineError(client: "internal_error", "\(error)")
    }
}
