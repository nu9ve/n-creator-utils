import Foundation
import Synchronization

/// How to launch an engine and how long to wait for it (nu9ve-engine-v1 §8). The app passes its
/// binary and arguments; nothing here knows an app.
public struct EngineConfiguration: Sendable {
    public enum StandardError: Sendable {
        /// stderr goes to `/dev/null`.
        case discard
        /// stderr is appended to a local log file.
        case file(URL)
    }

    /// Absolute path of the engine binary.
    public var executable: URL
    /// The mode and state folder, explicit (`["serve"]`, `["--state-dir", …, "rpc-serve"]`).
    public var arguments: [String]
    /// Nil inherits the app's environment.
    public var environment: [String: String]?
    public var workingDirectory: URL?
    public var standardError: StandardError = .discard
    /// The app's catalog (`<name>/<major>`). An engine that speaks another one is not used.
    public var expectedProtocol: String?
    /// For `engine_info` on a new process (§4 suggests 2 s).
    public var startupTimeout: Duration = .seconds(2)
    /// For each call, unless the call gives its own.
    public var requestTimeout: Duration = .seconds(30)
    /// A request line longer than this is refused before it is sent (§1).
    public var maxRequestBytes = 1 << 20
    /// A response line longer than this is discarded up to its newline and fails that call (§6.1).
    public var maxResponseBytes = 16 << 20
    /// The first restart after a crash is immediate; the next ones wait `backoff`, doubling up to
    /// `maximumBackoff` (§8 suggests 10 s to 320 s). A successful answer resets it.
    public var backoff: Duration = .seconds(10)
    public var maximumBackoff: Duration = .seconds(320)
    /// After closing stdin, how long the engine has before SIGTERM, and again before SIGKILL.
    public var shutdownGrace: Duration = .milliseconds(500)
    /// Answer `not_ported` (`method_unavailable`) without asking when a method is in neither
    /// `ported` nor `native`.
    public var checksMethods = true

    public init(executable: URL, arguments: [String], environment: [String: String]? = nil, expectedProtocol: String? = nil) {
        self.executable = executable
        self.arguments = arguments
        self.environment = environment
        self.expectedProtocol = expectedProtocol
    }
}

extension Duration {
    var dispatchInterval: DispatchTimeInterval {
        let (seconds, attoseconds) = components
        let nanoseconds = Double(seconds) * 1e9 + Double(attoseconds) / 1e9
        return .nanoseconds(Int(min(nanoseconds, Double(Int.max))))
    }
}

/// One engine process speaking NDJSON (§1, lines). Requests may be many at once; responses are
/// paired by id. It never restarts: `EngineClient` and `EngineLease` decide that.
final class EngineConnection: @unchecked Sendable {
    private struct Pending {
        let token: UInt64
        /// Nil for a raw request: it takes the next response in order.
        let id: EngineID?
        var continuation: CheckedContinuation<EngineEnvelope, any Error>?
        var dispatched = false
    }

    private struct State {
        var queue: [Pending] = []
        var closed: EngineError?
        var nextID = 0
        var nextToken: UInt64 = 0
        var cancelledEarly: Set<UInt64> = []
    }

    let configuration: EngineConfiguration
    private let process: Process
    private let input: FileHandle
    private let output: FileHandle
    private let state = Mutex(State())
    private let writer = DispatchQueue(label: "nu9ve.engine.writer")
    /// Called once, when the connection ends: the error it ended with, and whether it was asked to.
    private let onClose: @Sendable (EngineConnection, EngineError, Bool) -> Void

    var processIdentifier: Int32 { process.processIdentifier }
    var isClosed: Bool { state.withLock { $0.closed != nil } }

    private init(configuration: EngineConfiguration, process: Process, input: FileHandle, output: FileHandle,
                 onClose: @escaping @Sendable (EngineConnection, EngineError, Bool) -> Void) {
        self.configuration = configuration
        self.process = process
        self.input = input
        self.output = output
        self.onClose = onClose
    }

    static func launch(_ configuration: EngineConfiguration,
                       onClose: @escaping @Sendable (EngineConnection, EngineError, Bool) -> Void = { _, _, _ in })
        throws(EngineError) -> EngineConnection {
        let path = configuration.executable.path
        guard configuration.executable.isFileURL, path.hasPrefix("/"), FileManager.default.isExecutableFile(atPath: path) else {
            throw EngineError(client: EngineError.Code.engineMissing, "No engine binary at \(path).")
        }
        let process = Process(), stdin = Pipe(), stdout = Pipe()
        process.executableURL = configuration.executable
        process.arguments = configuration.arguments
        if let environment = configuration.environment { process.environment = environment }
        if let directory = configuration.workingDirectory { process.currentDirectoryURL = directory }
        process.standardInput = stdin
        process.standardOutput = stdout
        switch configuration.standardError {
        case .discard:
            process.standardError = FileHandle.nullDevice
        case .file(let url):
            if !FileManager.default.fileExists(atPath: url.path) {
                FileManager.default.createFile(atPath: url.path, contents: nil, attributes: [.posixPermissions: 0o600])
            }
            guard let log = try? FileHandle(forWritingTo: url) else {
                throw EngineError(client: EngineError.Code.startupFailed, "Cannot open the engine log.")
            }
            _ = try? log.seekToEnd()
            process.standardError = log
        }
        // A write to an engine that already exited fails with EPIPE instead of killing the app.
        _ = fcntl(stdin.fileHandleForWriting.fileDescriptor, F_SETNOSIGPIPE, 1)
        let connection = EngineConnection(configuration: configuration, process: process, input: stdin.fileHandleForWriting,
                                          output: stdout.fileHandleForReading, onClose: onClose)
        process.terminationHandler = { [weak connection] _ in
            connection?.close(EngineError(client: EngineError.Code.connectionLost, "The engine exited.", dispatched: true))
        }
        do {
            try process.run()
        } catch {
            throw EngineError(client: EngineError.Code.startupFailed, "The engine did not start: \(error.localizedDescription)")
        }
        let reader = Thread { [connection] in connection.readLoop() }
        reader.name = "nu9ve.engine.reader"
        reader.stackSize = 4 << 20
        reader.start()
        return connection
    }

    deinit {
        if process.isRunning { process.terminate() }
    }

    // MARK: Requests

    /// A request with the next integer id. Thrown errors are `EngineError`.
    func request(_ method: String, _ params: EngineObject, idempotencyKey: String? = nil, timeout: Duration) async throws -> EngineEnvelope {
        let id = state.withLock { state -> EngineID in
            state.nextID += 1
            return EngineID(state.nextID)
        }
        var message: EngineObject = ["id": id.value, "method": .string(method), "params": .object(params)]
        if let idempotencyKey { message["idempotency_key"] = .string(idempotencyKey) }
        let line = message.jsonLine
        guard line.count - 1 <= configuration.maxRequestBytes else {
            throw EngineError(client: EngineError.Code.requestTooLarge, "The request exceeds \(configuration.maxRequestBytes) bytes.")
        }
        return try await send(line, id: id, timeout: timeout)
    }

    /// Writes `text` and a newline as they are, and takes the next response in order, whatever its
    /// id. For conformance tests of an engine; a blank line gets no response, so use `writeRaw`.
    func sendRaw(_ text: Data, timeout: Duration) async throws -> EngineEnvelope {
        try await send(text + [0x0A], id: nil, timeout: timeout)
    }

    /// Writes `text` and a newline, expecting no response.
    func writeRaw(_ text: Data) {
        writer.async { [self] in try? input.write(contentsOf: text + [0x0A]) }
    }

    private func send(_ line: Data, id: EngineID?, timeout: Duration) async throws -> EngineEnvelope {
        let token = state.withLock { state -> UInt64 in
            state.nextToken += 1
            return state.nextToken
        }
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                writer.async { [self] in write(line, token: token, id: id, timeout: timeout, continuation: continuation) }
            }
        } onCancel: {
            cancel(token)
        }
    }

    /// On the writer queue, so requests are queued in the order they are written.
    private func write(_ line: Data, token: UInt64, id: EngineID?, timeout: Duration,
                       continuation: CheckedContinuation<EngineEnvelope, any Error>) {
        let refusal = state.withLock { state -> EngineError? in
            if let closed = state.closed { return closed.dispatched(false) }
            if state.cancelledEarly.remove(token) != nil {
                return EngineError(client: EngineError.Code.cancelled, "Cancelled before it was sent.")
            }
            state.queue.append(Pending(token: token, id: id, continuation: continuation))
            return nil
        }
        if let refusal { continuation.resume(throwing: refusal); return }
        DispatchQueue.global().asyncAfter(deadline: .now() + timeout.dispatchInterval) { [weak self] in self?.expire(token) }
        do {
            try input.write(contentsOf: line)
        } catch {
            close(EngineError(client: EngineError.Code.connectionLost, "Could not write to the engine.", dispatched: true))
            return
        }
        state.withLock { state in
            if let index = state.queue.firstIndex(where: { $0.token == token }) { state.queue[index].dispatched = true }
        }
    }

    /// The call stops waiting. Its entry stays, so the responses after it still pair in order.
    private func cancel(_ token: UInt64) {
        let found = state.withLock { state -> (CheckedContinuation<EngineEnvelope, any Error>, Bool)? in
            guard let index = state.queue.firstIndex(where: { $0.token == token }) else {
                state.cancelledEarly.insert(token)
                return nil
            }
            guard let continuation = state.queue[index].continuation else { return nil }
            state.queue[index].continuation = nil
            return (continuation, state.queue[index].dispatched)
        }
        if let (continuation, dispatched) = found {
            continuation.resume(throwing: EngineError(client: EngineError.Code.cancelled, "The call was cancelled.", dispatched: dispatched))
        }
    }

    /// A line engine answers in order, so one that does not answer in time blocks the rest:
    /// the process is closed, this call fails with `timeout` and the others with `connection_lost`.
    private func expire(_ token: UInt64) {
        let pending = state.withLock { state in state.queue.contains { $0.token == token && $0.continuation != nil } }
        guard pending else { return }
        close(EngineError(client: EngineError.Code.connectionLost, "The engine was restarted after a timeout.", dispatched: true),
              except: (token, EngineError(client: EngineError.Code.timeout, "The engine did not answer in time.", dispatched: true)))
    }

    // MARK: Reading

    private func readLoop() {
        let descriptor = output.fileDescriptor
        let limit = configuration.maxResponseBytes
        let chunkSize = 1 << 16
        let chunk = UnsafeMutablePointer<UInt8>.allocate(capacity: chunkSize)
        defer { chunk.deallocate() }
        var line = [UInt8]()
        var discarding = false
        while true {
            let count = read(descriptor, chunk, chunkSize)
            if count < 0, errno == EINTR { continue }
            if count <= 0 { break }
            var start = 0
            while start < count {
                let rest = UnsafeBufferPointer(start: chunk + start, count: count - start)
                let newline = rest.firstIndex(of: 0x0A)
                let end = newline.map { start + $0 } ?? count
                if !discarding {
                    if line.count + (end - start) > limit {
                        line.removeAll(keepingCapacity: false)
                        tooLarge()
                        discarding = newline == nil
                    } else {
                        line.append(contentsOf: UnsafeBufferPointer(start: chunk + start, count: end - start))
                        if newline != nil {
                            deliver(line)
                            line.removeAll(keepingCapacity: true)
                        }
                    }
                } else if newline != nil {
                    discarding = false
                }
                start = end + 1
            }
        }
        close(EngineError(client: EngineError.Code.connectionLost, "The engine closed its output.", dispatched: true))
    }

    private func deliver(_ line: [UInt8]) {
        guard line.contains(where: { ![0x20, 0x09, 0x0D].contains($0) }) else { return }
        switch EngineEnvelope.parse(Data(line)) {
        case .failure(let problem):
            close(EngineError(client: EngineError.Code.invalidResponse, "Invalid response envelope (\(problem.rawValue)).", dispatched: true))
        case .success(let envelope):
            route(envelope)
        }
    }

    /// The pending entry a response answers: the head for a raw request; else the one with its id
    /// (§2.1); else, for an error with a null id (a request the engine could not read), the head,
    /// since a line engine answers in order. Anything else breaks the protocol.
    private func route(_ envelope: EngineEnvelope) {
        let taken = state.withLock { state -> Pending? in
            guard let head = state.queue.first else { return nil }
            let index: Int?
            if head.id == nil {
                index = 0
            } else if let match = state.queue.firstIndex(where: { $0.id == envelope.id }) {
                index = match
            } else if envelope.id == .null, !envelope.ok {
                index = 0
            } else {
                index = nil
            }
            return index.map { state.queue.remove(at: $0) }
        }
        if let taken {
            taken.continuation?.resume(returning: envelope)
        } else {
            close(EngineError(client: EngineError.Code.invalidResponse, "Response id \(envelope.id) matches no request.", dispatched: true))
        }
    }

    private func tooLarge() {
        let head = state.withLock { state in state.queue.isEmpty ? nil : state.queue.removeFirst() }
        head?.continuation?.resume(throwing: EngineError(
            client: EngineError.Code.responseTooLarge, "The response exceeds \(configuration.maxResponseBytes) bytes.", dispatched: true))
    }

    // MARK: Closing

    /// Closes stdin, fails what is pending and stops the process after the grace periods (§8).
    /// `expected` says the app asked for it (a stop or a lease release), not a crash.
    func close(_ error: EngineError, except: (UInt64, EngineError)? = nil, expected: Bool = false) {
        let pending = state.withLock { state -> [Pending]? in
            guard state.closed == nil else { return nil }
            state.closed = error
            defer { state.queue = [] }
            return state.queue
        }
        guard let pending else { return }
        // The owner learns first, so a caller that wakes up below already sees the engine gone.
        onClose(self, error, expected)
        for entry in pending {
            let failure = entry.token == except?.0 ? except!.1 : error
            entry.continuation?.resume(throwing: failure.dispatched(entry.dispatched))
        }
        writer.async { [input] in try? input.close() }
        let process = process, grace = configuration.shutdownGrace.dispatchInterval
        DispatchQueue.global().asyncAfter(deadline: .now() + grace) {
            guard process.isRunning else { return }
            process.terminate()
            DispatchQueue.global().asyncAfter(deadline: .now() + grace) {
                if process.isRunning { kill(process.processIdentifier, SIGKILL) }
            }
        }
    }

    /// Closes stdin and lets the engine finish (§8). What is still pending fails with `cancelled`.
    func shutdown() {
        close(EngineError(client: EngineError.Code.cancelled, "The engine was stopped.", dispatched: true), expected: true)
    }
}

extension EngineObject {
    /// Compact JSON and a newline: one request line.
    var jsonLine: Data {
        var out = [UInt8]()
        EngineValue.write(.object(self), into: &out)
        out.append(0x0A)
        return Data(out)
    }
}
