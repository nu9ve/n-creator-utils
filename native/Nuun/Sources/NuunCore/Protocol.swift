import Foundation
import os

/// Slow reads (over 300 ms), to see what a card arrival waits behind: `log stream --predicate 'subsystem == "app.nu9ve.media"'`.
private let readerLog = Logger(subsystem: "app.nu9ve.media", category: "reader")

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
    /// Parse a JSON document in one pass. The Codable init above tries each type in turn and throws on every
    /// miss, which took ~6 s on a 6.9 MB snapshot; this gives the same cases: a whole number is `.integer`
    /// (or `.unsignedInteger` past Int64), any other number `.number`.
    public static func parse(_ data: Data) throws -> JSONValue {
        convert(try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]))
    }
    private static func convert(_ value: Any) -> JSONValue {
        switch value {
        case let object as [String: Any]: return .object(object.mapValues(convert))
        case let array as [Any]: return .array(array.map(convert))
        case let string as String: return .string(string)
        case let number as NSNumber:
            if CFGetTypeID(number) == CFBooleanGetTypeID() { return .bool(number.boolValue) }
            switch UInt8(number.objCType.pointee) {
            case UInt8(ascii: "Q"), UInt8(ascii: "L"), UInt8(ascii: "I"), UInt8(ascii: "S"), UInt8(ascii: "C"):
                let unsigned = number.uint64Value
                return unsigned > UInt64(Int64.max) ? .unsignedInteger(unsigned) : .integer(Int64(unsigned))
            case UInt8(ascii: "q"), UInt8(ascii: "l"), UInt8(ascii: "i"), UInt8(ascii: "s"), UInt8(ascii: "c"):
                return .integer(number.int64Value)
            default:
                let double = number.doubleValue
                if double.rounded() == double, abs(double) < 9.2e18 {
                    return .integer(Int64(double))
                }
                if number is NSDecimalNumber, let unsigned = UInt64(number.stringValue) { return .unsignedInteger(unsigned) }
                return .number(double)
            }
        default: return .null
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
        display = try JSONValue.parse(Data(document.utf8))
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
        try response(envelope: JSONValue.parse(data))
    }
    /// The same check on an envelope already parsed (the line server reads `id` from it first).
    public static func response(envelope: JSONValue) throws -> JSONValue {
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
    /// Unused since F5 (e) (2026-11-27): the app no longer runs Python. Kept so a `backend-config.json` of an
    /// older build still decodes; `script` still locates a checkout's `engine/target` in a build from source.
    public var python: String
    public var script: String
    public var stateDirectory: String?
    /// The Rust engine (`nuun-engine`), which answers everything.
    public var engine: String?
    /// Unused since F5 (e): there is one engine.
    public var reads: String?
    public var rustWrites: Bool?
    public init(python: String = "", script: String = "", stateDirectory: String? = nil, engine: String? = nil, reads: String? = nil, rustWrites: Bool? = nil) {
        self.python = python; self.script = script; self.stateDirectory = stateDirectory; self.engine = engine; self.reads = reads
        self.rustWrites = rustWrites
    }
    /// There is an engine to ask.
    public var usable: Bool {
        engine.map { FileManager.default.isExecutableFile(atPath: $0) } == true
    }
}

/// The environment of every engine process. In the test environment of scripts/sandbox.py (NU9VE_SANDBOX, with the
/// catalog inside it), HOME points inside it too: Stalker libraries, the missions inbox (~/Downloads) and Clipper's
/// projects are the environment's, never the person's. The app itself keeps its own HOME and settings.
public enum EngineEnvironment {
    public static let values: [String: String] = {
        var env = ProcessInfo.processInfo.environment
        if let root = env["NU9VE_SANDBOX"], let home = env["NU9VE_MEDIA_HOME"], home.hasPrefix(root + "/") {
            env["HOME"] = root + "/home"
            env["CLIPPER_PROJECTS_ROOT"] = root + "/home/Downloads/clipper-projects"
            // The suite agent and its files are QA ones too (nu9ve-agent-v1 §1): no LaunchAgent, no keychain, and
            // nothing in this Mac's agent, device.json or engine registry.
            let suite = root + "/home/Library/Application Support/nu9ve"
            for (key, path) in [("NU9VE_AGENT_DIR", "/agent"), ("NU9VE_DEVICE_FILE", "/device.json"),
                                ("NU9VE_ENGINES_DIR", "/engines"), ("NU9VE_VOLUMES_DIR", "/volumes")] where env[key] == nil {
                env[key] = suite + path
            }
        }
        // The version in the engine's suite record (nu9ve-registry-v1 §2): the app's, as its «Acerca de».
        if let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String { env["NUUN_APP_VERSION"] = version }
        return env
    }()
}

/// One long-lived child that answers one JSON line per request (`rpc-serve` of either engine).
final class LineServer: @unchecked Sendable {
    let executable: String
    let arguments: [String]
    private let lock = NSLock()
    private var process: Process?
    private var input: FileHandle?
    private var buffer = Data()
    /// Bytes of `buffer` already searched for a newline: a 7 MB reply arrives in many pipe chunks, and searching
    /// the whole buffer again on each one cost seconds.
    private var scanned = 0
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
        process = nil; input = nil; pending = [:]; buffer = Data(); scanned = 0
        lock.unlock()
        try? handle?.close()
        // EOF first: the engine leaves in order and removes its suite record (nu9ve-registry-v1 §5). A child that
        // is still there after half a second is terminated, as before.
        if let running {
            let deadline = Date().addingTimeInterval(0.5)
            while running.isRunning && Date() < deadline { usleep(10_000) }
            if running.isRunning { running.terminate() }
        }
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
        child.environment = EngineEnvironment.values
        child.standardInput = stdin; child.standardOutput = stdout; child.standardError = FileHandle.nullDevice
        child.qualityOfService = .userInitiated
        stdout.fileHandleForReading.readabilityHandler = { [weak self, weak child] handle in
            let data = handle.availableData
            if data.isEmpty { handle.readabilityHandler = nil; if let child { self?.stop(only: child) }; return }
            self?.receive(data)
        }
        child.terminationHandler = { [weak self] ended in self?.stop(only: ended) }
        try child.run()
        process = child; input = stdin.fileHandleForWriting; buffer = Data(); scanned = 0
        return stdin.fileHandleForWriting
    }

    private func receive(_ data: Data) {
        lock.lock()
        buffer.append(data)
        var ready: [(CheckedContinuation<JSONValue, Error>, Result<JSONValue, Error>)] = []
        var lines: [Data] = []
        var start = buffer.startIndex
        var from = buffer.startIndex + scanned
        // Only the new bytes are searched; whole lines leave the buffer in one copy at the end.
        while let newline = buffer[from...].firstIndex(of: 0x0A) {
            lines.append(Data(buffer[start..<newline]))
            start = newline + 1
            from = start
        }
        if start > buffer.startIndex { buffer = Data(buffer[start...]) }
        scanned = buffer.count
        for line in lines {
            guard let envelope = try? JSONValue.parse(line) else { continue }
            let id = Int(envelope["id"].double)
            guard let continuation = pending.removeValue(forKey: id) else { continue }
            ready.append((continuation, Result { try ProtocolBoundary.response(envelope: envelope) }))
        }
        lock.unlock()
        for (continuation, result) in ready { continuation.resume(with: result) }
    }
}

/// The long-lived readers (snapshot, progress, suggestions), answered by `nuun-engine rpc-serve`. Since F5 (e)
/// (2026-11-27) there is no Python behind it. A read the server does not keep but the engine answers in one shot
/// (`engine_info.once`, such as `resolve`) goes to a one-shot `rpc`; everything else goes to the server, which
/// refuses a mutation (`read_only`), so no write ever starts here.
public final class PersistentReader: @unchecked Sendable {
    public let configuration: BackendConfiguration
    private let rust: LineServer?
    private let once: BackendClient
    private let lock = NSLock()
    private var onceMethods: Set<String>?
    private var answered: [String: String] = [:]

    public init(configuration: BackendConfiguration) {
        self.configuration = configuration
        let state = configuration.stateDirectory.map { ["--state-dir", $0] } ?? []
        rust = configuration.usable ? configuration.engine.map { LineServer(executable: $0, arguments: state + ["rpc-serve"]) } : nil
        once = BackendClient(configuration: configuration)
    }
    deinit { stop() }

    /// How each method was answered last (`rpc-serve` or `rpc`), for Ajustes → Diagnóstico.
    public var answeredBy: [String: String] { lock.withLock { answered } }

    public func call(_ method: String, _ params: [String: JSONValue] = [:]) async throws -> JSONValue {
        let started = ContinuousClock.now
        defer {
            let d = started.duration(to: .now).components
            let ms = d.seconds * 1000 + d.attoseconds / 1_000_000_000_000_000
            if ms > 300 { readerLog.notice("\(method, privacy: .public) took \(ms, privacy: .public) ms") }
        }
        guard let rust else { throw BackendFailure(code: "runtime_missing", message: String(localized: "Esta instalación no tiene el motor de nüün.")) }
        if await answeredOnce(method, rust) {
            record(method, "rpc")
            return try await once.call(method, params)
        }
        record(method, "rpc-serve")
        return try await rust.send(method, params)
    }

    public func stop() { rust?.stop() }

    private func record(_ method: String, _ how: String) { lock.withLock { answered[method] = how } }

    /// Whether `method` is a one-shot read the server does not keep (`engine_info`, asked once).
    private func answeredOnce(_ method: String, _ rust: LineServer) async -> Bool {
        if let known = lock.withLock({ onceMethods }) { return known.contains(method) }
        let info = try? await rust.send("engine_info", [:])
        let served = Set((info?["ported"].array ?? []).map(\.string))
        let writes = Set((info?["writes"].array ?? []).map(\.string))
        // A method that may write never goes to the one-shot process from here.
        let methods = Set((info?["once"].array ?? []).map(\.string)).subtracting(served).subtracting(writes)
        lock.withLock { onceMethods = methods }
        return methods.contains(method)
    }
}

final class CollectedData: @unchecked Sendable {
    private let lock = NSLock()
    private var value = Data()
    func store(_ data: Data) { lock.lock(); value = data; lock.unlock() }
    func read() -> Data { lock.lock(); defer { lock.unlock() }; return value }
}

/// Every one-shot call, reads and writes, in its own `nuun-engine rpc` process. Since F5 (e) there is no Python:
/// what the engine refuses is the answer.
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
                do {
                    guard self.configuration.usable, let engine = self.configuration.engine else {
                        throw BackendFailure(code: "runtime_missing", message: String(localized: "Esta instalación no tiene el motor de nüün."))
                    }
                    continuation.resume(returning: try self.execute(engine, self.state + ["rpc"], request, epoch: epoch))
                }
                catch { continuation.resume(throwing: error) }
            }
        }
    }
    private func currentEpoch() -> UInt { lock.lock(); defer { lock.unlock() }; return cancelEpoch }

    private var state: [String] { configuration.stateDirectory.map { ["--state-dir", $0] } ?? [] }

    private func execute(_ executable: String, _ arguments: [String], _ request: Data, epoch: UInt) throws -> JSONValue {
        guard FileManager.default.isExecutableFile(atPath: executable) else { throw BackendFailure(code: "runtime_missing", message: "Runtime no configurado") }
        let process = Process(), input = Pipe(), output = Pipe(), errors = Pipe()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = arguments
        process.environment = EngineEnvironment.values
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
