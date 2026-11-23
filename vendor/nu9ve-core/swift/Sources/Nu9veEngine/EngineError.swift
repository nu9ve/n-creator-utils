import Foundation

/// A failure of a call: an error the engine answered (`origin == .engine`, the method's own
/// code such as `project_not_found`) or one this client produced (§7, `origin == .client`).
public struct EngineError: Error, Sendable, Equatable, CustomStringConvertible {
    public enum Origin: Sendable, Equatable {
        case engine
        case client
    }

    public let code: String
    public let message: String
    public let origin: Origin
    /// The request reached the engine whole. After a timeout or a lost connection with this set,
    /// a write may have happened: ask its status, do not resend it blindly (§6.3).
    public let dispatched: Bool
    /// The engine's whole `error` object, with `reason`, `retryable` and the app's fields.
    public let details: EngineObject

    public var reason: String? { details["reason"]?.stringValue }
    public var retryable: Bool? { details["retryable"]?.boolValue }
    public var retryAfterSeconds: Int? { details["retry_after_seconds"]?.numberValue?.int }
    public var description: String { "\(code): \(message)" }

    public init(engine error: EngineObject) {
        code = error["code"]?.stringValue ?? "internal_error"
        message = error["message"]?.stringValue ?? ""
        origin = .engine
        dispatched = true
        details = error
    }

    public init(client code: String, _ message: String, dispatched: Bool = false, details: EngineObject = [:]) {
        self.code = code
        self.message = message
        origin = .client
        self.dispatched = dispatched
        var details = details
        details["code"] = .string(code)
        details["message"] = .string(message)
        self.details = details
    }

    func dispatched(_ flag: Bool) -> EngineError {
        EngineError(code: code, message: message, origin: origin, dispatched: flag, details: details)
    }

    private init(code: String, message: String, origin: Origin, dispatched: Bool, details: EngineObject) {
        self.code = code
        self.message = message
        self.origin = origin
        self.dispatched = dispatched
        self.details = details
    }
}

extension EngineError {
    /// The codes this client produces (§7). The rest come from the engine.
    public enum Code {
        public static let engineMissing = "engine_missing"
        public static let startupFailed = "startup_failed"
        public static let startupTimeout = "startup_timeout"
        public static let incompatibleEngine = "incompatible_engine"
        public static let timeout = "timeout"
        public static let connectionLost = "connection_lost"
        /// Waiting out the backoff after a crash; `retry_after_seconds` says how long.
        public static let engineUnavailable = "engine_unavailable"
        /// The engine broke the protocol: an invalid envelope or an id nobody asked for.
        public static let invalidResponse = "invalid_response"
        public static let requestTooLarge = "request_too_large"
        public static let responseTooLarge = "response_too_large"
        public static let invalidRequest = "invalid_request"
        public static let cancelled = "cancelled"
        /// A lease was asked while another one is held by this lease object, or after it ended.
        public static let leaseEnded = "lease_ended"
    }
}

/// «This engine cannot answer this yet, and did nothing» (§5). A result, not an error: the app
/// may ask its oracle, or tell the person the feature needs the other engine.
public struct EngineNotPorted: Sendable, Equatable {
    public let reason: String
    public let message: String
    /// The client decided it without asking: the method is in neither `ported` nor `native`.
    public let answeredByClient: Bool
}

/// What a call gives back when it did not fail: the value, or `not_ported`.
public enum EngineOutcome<Value: Sendable>: Sendable {
    case value(Value)
    case notPorted(EngineNotPorted)

    public var value: Value? { if case .value(let value) = self { return value }; return nil }
    public var notPorted: EngineNotPorted? { if case .notPorted(let info) = self { return info }; return nil }

    public func map<T: Sendable>(_ transform: (Value) throws -> T) rethrows -> EngineOutcome<T> {
        switch self {
        case .value(let value): .value(try transform(value))
        case .notPorted(let info): .notPorted(info)
        }
    }
}

/// `data` of a successful response, parsed and as the engine wrote it.
public struct EngineReply: Sendable {
    public let value: EngineValue
    /// The exact bytes of `data`, for `JSONDecoder` or for storing without re-encoding.
    public let bytes: Data

    public func decode<T: Decodable>(_ type: T.Type = T.self, decoder: JSONDecoder = JSONDecoder()) throws -> T {
        try decoder.decode(type, from: bytes)
    }
}
