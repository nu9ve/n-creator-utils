import Foundation

public enum JSONValue: Codable, Sendable, Equatable {
    case object([String: JSONValue]), array([JSONValue]), string(String), integer(Int64), unsignedInteger(UInt64), number(Double), bool(Bool), null
    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null }
        else if let v = try? c.decode(Bool.self) { self = .bool(v) }
        else if let v = try? c.decode(String.self) { self = .string(v) }
        else if let v = try? c.decode(Int64.self) { self = .integer(v) }
        else if let v = try? c.decode(UInt64.self) { self = .unsignedInteger(v) }
        else if let v = try? c.decode(Double.self) { self = .number(v) }
        else if let v = try? c.decode([String: JSONValue].self) { self = .object(v) }
        else { self = .array(try c.decode([JSONValue].self)) }
    }
    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .object(let v): try c.encode(v)
        case .array(let v): try c.encode(v)
        case .string(let v): try c.encode(v)
        case .integer(let v): try c.encode(v)
        case .unsignedInteger(let v): try c.encode(v)
        case .number(let v): try c.encode(v)
        case .bool(let v): try c.encode(v)
        case .null: try c.encodeNil()
        }
    }
    public subscript(_ key: String) -> JSONValue { if case .object(let o) = self { return o[key] ?? .null }; return .null }
    public var string: String { if case .string(let s) = self { return s }; return "" }
    public var bool: Bool { if case .bool(let b) = self { return b }; return false }
    public var double: Double {
        switch self { case .integer(let i): return Double(i); case .unsignedInteger(let i): return Double(i); case .number(let d): return d; default: return 0 }
    }
    public var array: [JSONValue] { if case .array(let a) = self { return a }; return [] }
    public var object: [String: JSONValue] { if case .object(let o) = self { return o }; return [:] }
}

/// The display tree is deliberately separate from the original reviewed JSON document.
/// Re-encoding the tree can change integral floats or numbers outside Int64.
public struct ReviewedPlan: Sendable {
    public let document: String
    public let display: JSONValue
    public init(document: String) throws {
        self.document = document
        display = try JSONDecoder().decode(JSONValue.self, from: Data(document.utf8))
        guard !display["digest"].string.isEmpty else {
            throw BackendFailure(code: "invalid_plan", message: "El plan recibido no tiene digest")
        }
    }
    public var applyParameters: [String: JSONValue] {
        ["plan": .string(document), "digest": display["digest"]]
    }
}

public struct BackendFailure: LocalizedError, Sendable {
    public let code: String
    public let message: String
    public var errorDescription: String? { message }
    public init(code: String, message: String) { self.code = code; self.message = message }
}

public enum ProtocolBoundary {
    public static func response(_ data: Data) throws -> JSONValue {
        let envelope = try JSONDecoder().decode(JSONValue.self, from: data)
        guard envelope["ok"].bool else {
            throw BackendFailure(code: envelope["error"]["code"].string, message: envelope["error"]["message"].string)
        }
        return envelope["data"]
    }
    public static func shouldOfferCard(destinationsOnline: Bool, sourceIsDestination: Bool, removableExternal: Bool) -> Bool {
        destinationsOnline && !sourceIsDestination && removableExternal
    }
}

public struct BackendConfiguration: Codable, Sendable {
    public var python: String
    public var script: String
    public var stateDirectory: String?
    /// The Rust engine (`nuun-engine`), when this build has one.
    public var engine: String?
    /// Who answers reads: `"rust"` (default when an engine exists; Python answers whatever Rust does not have)
    /// or `"python"`. Mutations always go to Python on this Mac until the Rust engine's physical run.
    public var reads: String?
    public init(python: String, script: String, stateDirectory: String? = nil, engine: String? = nil, reads: String? = nil) {
        self.python = python; self.script = script; self.stateDirectory = stateDirectory; self.engine = engine; self.reads = reads
    }
    public var usable: Bool {
        FileManager.default.isExecutableFile(atPath: python) && FileManager.default.fileExists(atPath: script)
    }
    /// Reads go to the Rust engine first.
    public var rustReads: Bool {
        reads != "python" && engine.map { FileManager.default.isExecutableFile(atPath: $0) } == true
    }
}

/// One long-lived child that answers one JSON line per request (`rpc-serve` of either engine).
final class LineServer: @unchecked Sendable {
    let executable: String
    let arguments: [String]
    private let lock = NSLock()
    private var process: Process?
    private var input: FileHandle?
    private var buffer = Data()
    private var nextID = 0
    private var pending: [Int: CheckedContinuation<JSONValue, Error>] = [:]

    init(executable: String, arguments: [String]) { self.executable = executable; self.arguments = arguments }
    deinit { stop() }

    func stop() { stop(only: nil) }

    /// `only` keeps a late termination of an old child from stopping its replacement.
    private func stop(only child: Process?) {
        lock.lock()
        if let child, child !== process { lock.unlock(); return }
        let running = process, handle = input, waiting = pending
        process = nil; input = nil; pending = [:]; buffer = Data()
        lock.unlock()
        try? handle?.close()
        if let running, running.isRunning { running.terminate() }
        for continuation in waiting.values {
            continuation.resume(throwing: BackendFailure(code: "reader_unavailable", message: "Lector reiniciado"))
        }
    }

    func send(_ method: String, _ params: [String: JSONValue]) async throws -> JSONValue {
        try await withCheckedThrowingContinuation { continuation in
            lock.lock()
            do {
                let handle = try startLocked()
                nextID += 1
                let id = nextID
                var line = try JSONEncoder().encode(JSONValue.object(["id": .integer(Int64(id)), "method": .string(method), "params": .object(params)]))
                line.append(0x0A)
                pending[id] = continuation
                lock.unlock()
                do { try handle.write(contentsOf: line) }
                catch { stop(only: process) }
            } catch {
                lock.unlock()
                continuation.resume(throwing: BackendFailure(code: "reader_unavailable", message: error.localizedDescription))
            }
        }
    }

    private func startLocked() throws -> FileHandle {
        if let process, process.isRunning, let input { return input }
        guard FileManager.default.isExecutableFile(atPath: executable) else { throw BackendFailure(code: "runtime_missing", message: "Runtime no configurado") }
        let child = Process(), stdin = Pipe(), stdout = Pipe()
        child.executableURL = URL(fileURLWithPath: executable)
        child.arguments = arguments
        child.standardInput = stdin; child.standardOutput = stdout; child.standardError = FileHandle.nullDevice
        child.qualityOfService = .userInitiated
        stdout.fileHandleForReading.readabilityHandler = { [weak self, weak child] handle in
            let data = handle.availableData
            if data.isEmpty { handle.readabilityHandler = nil; if let child { self?.stop(only: child) }; return }
            self?.receive(data)
        }
        child.terminationHandler = { [weak self] ended in self?.stop(only: ended) }
        try child.run()
        process = child; input = stdin.fileHandleForWriting; buffer = Data()
        return stdin.fileHandleForWriting
    }

    private func receive(_ data: Data) {
        lock.lock()
        buffer.append(data)
        var ready: [(CheckedContinuation<JSONValue, Error>, Result<JSONValue, Error>)] = []
        while let newline = buffer.firstIndex(of: 0x0A) {
            let line = buffer[buffer.startIndex..<newline]
            buffer.removeSubrange(buffer.startIndex...newline)
            guard let envelope = try? JSONDecoder().decode(JSONValue.self, from: Data(line)) else { continue }
            let id = Int(envelope["id"].double)
            guard let continuation = pending.removeValue(forKey: id) else { continue }
            ready.append((continuation, Result { try ProtocolBoundary.response(Data(line)) }))
        }
        lock.unlock()
        for (continuation, result) in ready { continuation.resume(with: result) }
    }
}

/// The long-lived readers (snapshot, progress, suggestions). With a Rust engine, each read it has
/// (`engine_info.ported`) goes to `nuun-engine rpc-serve` first; whatever it does not have, or any
/// transport failure, is asked of Python's `rpc-serve`, so a read never goes unanswered. A Rust answer
/// that is an error of the method itself (unknown project, offline disk…) is the answer, as Python's would be:
/// the two engines give the same answers (scripts/engine_shadow.py). Mutations stay one Python process per
/// request so cancelling interrupts exactly that operation.
public final class PersistentReader: @unchecked Sendable {
    public let configuration: BackendConfiguration
    /// Rust codes that mean «ask Python instead».
    static let fallbackCodes: Set<String> = ["not_ported", "reader_unavailable", "runtime_missing"]
    private let python: LineServer
    private let rust: LineServer?
    private let fallback: BackendClient
    private let lock = NSLock()
    private var rustMethods: Set<String>?
    private var answered: [String: String] = [:]

    public init(configuration: BackendConfiguration) {
        self.configuration = configuration
        let state = configuration.stateDirectory.map { ["--state-dir", $0] } ?? []
        python = LineServer(executable: configuration.python, arguments: ["-S", configuration.script, "media"] + state + ["rpc-serve"])
        rust = configuration.rustReads ? configuration.engine.map { LineServer(executable: $0, arguments: state + ["rpc-serve"]) } : nil
        fallback = BackendClient(configuration: configuration)
    }
    deinit { stop() }

    /// Which engine answered each method last (`rust` or `python`), for Ajustes → Diagnóstico.
    public var answeredBy: [String: String] { lock.withLock { answered } }

    public func call(_ method: String, _ params: [String: JSONValue] = [:]) async throws -> JSONValue {
        if let rust, await rustAnswers(method, rust) {
            do {
                let value = try await rust.send(method, params)
                record(method, "rust")
                return value
            } catch let failure as BackendFailure where !Self.fallbackCodes.contains(failure.code) {
                record(method, "rust")
                throw failure
            } catch {
                // Python answers what Rust could not.
            }
        }
        record(method, "python")
        guard configuration.usable else { throw BackendFailure(code: "runtime_missing", message: "Runtime no configurado") }
        do { return try await python.send(method, params) }
        catch let failure as BackendFailure where failure.code != "reader_unavailable" { throw failure }
        catch { return try await fallback.call(method, params) }
    }

    public func stop() { python.stop(); rust?.stop() }

    private func record(_ method: String, _ engine: String) { lock.withLock { answered[method] = engine } }

    /// The methods the engine answers here, asked once (`engine_info`); none if it cannot say.
    private func rustAnswers(_ method: String, _ rust: LineServer) async -> Bool {
        if let known = lock.withLock({ rustMethods }) { return known.contains(method) }
        let info = try? await rust.send("engine_info", [:])
        let methods = Set((info?["ported"].array ?? []).map(\.string))
        lock.withLock { rustMethods = methods }
        return methods.contains(method)
    }
}

private final class CollectedData: @unchecked Sendable {
    private let lock = NSLock()
    private var value = Data()
    func store(_ data: Data) { lock.lock(); value = data; lock.unlock() }
    func read() -> Data { lock.lock(); defer { lock.unlock() }; return value }
}

public final class BackendClient: @unchecked Sendable {
    public let configuration: BackendConfiguration
    private let lock = NSLock()
    private var processes: [UUID: Process] = [:]
    private var cancelEpoch: UInt = 0
    public init(configuration: BackendConfiguration) { self.configuration = configuration }
    public func cancel() {
        lock.lock(); cancelEpoch &+= 1; let active = Array(processes.values); lock.unlock()
        for process in active where process.isRunning { process.interrupt() }
    }
    public func call(_ method: String, _ params: [String: JSONValue] = [:]) async throws -> JSONValue {
        let request = try JSONEncoder().encode(JSONValue.object(["method": .string(method), "params": .object(params)]))
        let epoch = currentEpoch()
        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .utility).async {
                do { continuation.resume(returning: try self.run(request, epoch: epoch)) }
                catch { continuation.resume(throwing: error) }
            }
        }
    }
    private func currentEpoch() -> UInt { lock.lock(); defer { lock.unlock() }; return cancelEpoch }
    private func run(_ request: Data, epoch: UInt) throws -> JSONValue {
        guard configuration.usable else { throw BackendFailure(code: "runtime_missing", message: "Configura el runtime Python y nu9ve.py antes de operar.") }
        let process = Process(), input = Pipe(), output = Pipe(), errors = Pipe()
        process.executableURL = URL(fileURLWithPath: configuration.python)
        process.arguments = ["-S", configuration.script, "media"] + (configuration.stateDirectory.map { ["--state-dir", $0] } ?? []) + ["rpc"]
        process.standardInput = input; process.standardOutput = output; process.standardError = errors
        process.qualityOfService = .utility
        let id = UUID()
        lock.lock()
        guard epoch == cancelEpoch else { lock.unlock(); throw BackendFailure(code: "cancelled", message: "Operación cancelada antes de iniciar.") }
        do { try process.run(); processes[id] = process; lock.unlock() }
        catch { lock.unlock(); throw error }
        defer { lock.lock(); processes.removeValue(forKey: id); lock.unlock() }
        let out = CollectedData(), err = CollectedData(), group = DispatchGroup()
        group.enter()
        DispatchQueue.global(qos: .utility).async { out.store(output.fileHandleForReading.readDataToEndOfFile()); group.leave() }
        group.enter()
        DispatchQueue.global(qos: .utility).async { err.store(errors.fileHandleForReading.readDataToEndOfFile()); group.leave() }
        try input.fileHandleForWriting.write(contentsOf: request)
        try input.fileHandleForWriting.close()
        process.waitUntilExit(); group.wait()
        let data = out.read()
        guard data.count <= 32 * 1024 * 1024 else { throw BackendFailure(code: "response_too_large", message: "La respuesta excede el límite permitido.") }
        guard !data.isEmpty else { throw BackendFailure(code: "backend_failed", message: "El motor terminó sin respuesta. Revisa runtime/permisos.") }
        return try ProtocolBoundary.response(data)
    }
}
