import Foundation

public enum JSONValue: Codable, Sendable, Equatable {
    case object([String: JSONValue]), array([JSONValue]), string(String), integer(Int64), number(Double), bool(Bool), null
    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null }
        else if let v = try? c.decode(Bool.self) { self = .bool(v) }
        else if let v = try? c.decode(String.self) { self = .string(v) }
        else if let v = try? c.decode(Int64.self) { self = .integer(v) }
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
        case .number(let v): try c.encode(v)
        case .bool(let v): try c.encode(v)
        case .null: try c.encodeNil()
        }
    }
    public subscript(_ key: String) -> JSONValue { if case .object(let o) = self { return o[key] ?? .null }; return .null }
    public var string: String { if case .string(let s) = self { return s }; return "" }
    public var bool: Bool { if case .bool(let b) = self { return b }; return false }
    public var array: [JSONValue] { if case .array(let a) = self { return a }; return [] }
    public var object: [String: JSONValue] { if case .object(let o) = self { return o }; return [:] }
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
    public init(python: String, script: String, stateDirectory: String? = nil) { self.python = python; self.script = script; self.stateDirectory = stateDirectory }
    public var usable: Bool {
        FileManager.default.isExecutableFile(atPath: python) && FileManager.default.fileExists(atPath: script)
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
