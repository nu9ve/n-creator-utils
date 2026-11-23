import Foundation
import Synchronization

/// A session lease (nu9ve-engine-v1 §6.4): a method whose answer means «held», and whose
/// resource (locks, a player's source) stays held while its process lives. Each lease gets a
/// process of its own, never restarted: if it dies, the lease is lost and `onEnd` says so.
/// `release()` sends the app's release method on that same process and then closes it; closing
/// without releasing (or the app quitting) ends the process with EOF, which also releases.
public final class EngineLease: Sendable {
    public let info: EngineInfo
    /// `data` of the acquisition.
    public let acquired: EngineReply
    public let releaseMethod: String

    private let connection: EngineConnection
    private let released = Mutex(false)

    private init(info: EngineInfo, acquired: EngineReply, releaseMethod: String, connection: EngineConnection) {
        self.info = info
        self.acquired = acquired
        self.releaseMethod = releaseMethod
        self.connection = connection
    }

    deinit {
        connection.shutdown()
    }

    /// The process still lives and the lease was not released.
    public var isHeld: Bool { !connection.isClosed && !released.withLock { $0 } }

    /// The engine's process id, for diagnostics.
    public var processIdentifier: Int32 { connection.processIdentifier }

    static func open(_ configuration: EngineConfiguration, _ method: String, _ params: EngineObject, release: String,
                     timeout: Duration?, onEnd: (@Sendable (EngineError) -> Void)?) async throws(EngineError) -> EngineOutcome<EngineLease> {
        try await engineErrors {
            let connection = try EngineConnection.launch(configuration) { _, error, expected in
                if !expected { onEnd?(error) }
            }
            do {
                let info = try await EngineClient.handshake(connection, configuration)
                if configuration.checksMethods, !info.answers(method) || !info.answers(release) {
                    connection.shutdown()
                    return .notPorted(EngineNotPorted(reason: "method_unavailable",
                                                      message: "The engine does not list \(method) and \(release).",
                                                      answeredByClient: true))
                }
                let envelope = try await connection.request(method, params, timeout: timeout ?? configuration.requestTimeout)
                switch try EngineClient.outcome(envelope) {
                case .value(let reply):
                    return .value(EngineLease(info: info, acquired: reply, releaseMethod: release, connection: connection))
                case .notPorted(let missing):
                    connection.shutdown()
                    return .notPorted(missing)
                }
            } catch {
                connection.shutdown()
                throw error
            }
        }
    }

    /// Sends the release method with empty params, waits for its answer and ends the process.
    @discardableResult
    public func release(timeout: Duration? = nil) async throws(EngineError) -> EngineReply {
        let first = released.withLock { state in
            defer { state = true }
            return !state
        }
        guard first, !connection.isClosed else {
            throw EngineError(client: EngineError.Code.leaseEnded, "The lease already ended.")
        }
        defer { connection.shutdown() }
        return try await engineErrors {
            let envelope = try await connection.request(releaseMethod, [:], timeout: timeout ?? connection.configuration.requestTimeout)
            guard case .value(let reply) = try EngineClient.outcome(envelope) else {
                throw EngineError(client: EngineError.Code.invalidResponse, "The release answered not_ported.", dispatched: true)
            }
            return reply
        }
    }

    /// Ends the process without the release method: the engine releases on EOF.
    public func close() {
        released.withLock { $0 = true }
        connection.shutdown()
    }
}
