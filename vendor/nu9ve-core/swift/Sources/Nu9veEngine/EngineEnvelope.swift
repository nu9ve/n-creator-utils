import Foundation

/// A request or response id (nu9ve-engine-v1 §2.1): an integer as written, a string, or null.
/// `7` and `"7"` are different ids, and so are `7` and `7.0` (which is not an id).
public enum EngineID: Sendable, Hashable, CustomStringConvertible {
    case integer(String)
    case string(String)
    case null

    public init(_ value: Int) { self = .integer(String(value)) }

    /// Nil for a float, a bool, an object or an array.
    public init?(_ value: EngineValue) {
        switch value {
        case .null: self = .null
        case .string(let text): self = .string(text)
        case .number(let number) where number.isInteger: self = .integer(number.text == "-0" ? "0" : number.text)
        default: return nil
        }
    }

    public var value: EngineValue {
        switch self {
        case .integer(let digits): .number(EngineNumber(unchecked: digits))
        case .string(let text): .string(text)
        case .null: .null
        }
    }

    public var description: String { value.jsonText }
}

/// Why a response envelope is not valid nu9ve-engine-v1 §3. The raw values are the `why` of
/// `data/engine-examples-v1.json`.
public enum EnvelopeProblem: String, Error, Sendable {
    case notJSON = "not_json"
    case notObject = "not_object"
    case badID = "bad_id"
    case badOK = "bad_ok"
    case dataAndError = "data_and_error"
    case badCode = "bad_code"
    case badMessage = "bad_message"
    case notPortedWithoutReason = "not_ported_without_reason"
}

/// A response the engine wrote, already checked against §3.
public struct EngineEnvelope: Sendable {
    public enum Body: Sendable {
        /// `data` and its exact bytes as the engine wrote them.
        case data(EngineValue, bytes: Data)
        /// The whole `error` object; `code` and `message` are checked.
        case error(EngineObject)
    }

    public let id: EngineID
    public let body: Body

    public var ok: Bool { if case .data = body { return true }; return false }

    public static func parse(_ bytes: Data) -> Result<EngineEnvelope, EnvelopeProblem> {
        guard let parsed = try? EngineJSONParser.parse(bytes) else { return .failure(.notJSON) }
        guard case .object(let object) = parsed.value else { return .failure(.notObject) }
        guard let rawID = object["id"], let id = EngineID(rawID) else { return .failure(.badID) }
        guard case .bool(let ok)? = object["ok"] else { return .failure(.badOK) }
        if ok {
            guard let data = object["data"], object["error"] == nil, let range = parsed.members["data"] else {
                return .failure(.dataAndError)
            }
            return .success(EngineEnvelope(id: id, body: .data(data, bytes: bytes.subdata(in: shifted(range, bytes)))))
        }
        guard object["data"] == nil, case .object(let error)? = object["error"] else { return .failure(.dataAndError) }
        guard case .string(let code)? = error["code"], isToken(code, limit: 64) else { return .failure(.badCode) }
        guard case .string? = error["message"] else { return .failure(.badMessage) }
        if code == "not_ported" {
            guard case .string(let reason)? = error["reason"], isToken(reason, limit: 40) else {
                return .failure(.notPortedWithoutReason)
            }
        }
        return .success(EngineEnvelope(id: id, body: .error(error)))
    }

    /// `^[a-z_]{1,limit}$`, the shape of `error.code` (64) and `error.reason` (40).
    public static func isToken(_ text: String, limit: Int) -> Bool {
        let bytes = text.utf8
        return (1...limit).contains(bytes.count) && bytes.allSatisfy { (0x61...0x7A).contains($0) || $0 == 0x5F }
    }

    private static func shifted(_ range: Range<Int>, _ data: Data) -> Range<Data.Index> {
        (data.startIndex + range.lowerBound)..<(data.startIndex + range.upperBound)
    }
}

/// Why an `engine_info` is not usable (§4). The raw values are the `why` of the examples.
public enum EngineInfoProblem: String, Error, Sendable {
    case notObject = "not_object"
    case missingContract = "missing_contract"
    case missingProtocol = "missing_protocol"
    case missingEngine = "missing_engine"
    case missingVersion = "missing_version"
    case otherContract = "other_contract"
    case badProtocol = "bad_protocol"
    case missingCapabilities = "missing_capabilities"
    case missingPorted = "missing_ported"
    case missingNative = "missing_native"
    case portedAndNativeOverlap = "ported_and_native_overlap"
}

/// The handshake (§4), asked once per process.
public struct EngineInfo: Sendable {
    public static let contract = "nu9ve-engine/1"

    public let contract: String
    /// The app's catalog, `<name>/<major>`.
    public let `protocol`: String
    public let engine: String
    public let version: String
    public let capabilities: [String]
    public let ported: [String]
    public let native: [String]
    /// The whole object, with the app's own fields.
    public let fields: EngineObject

    /// This engine answers the method (it is in `ported` or `native`).
    public func answers(_ method: String) -> Bool { ported.contains(method) || native.contains(method) }

    public var limits: EngineObject? { fields["limits"]?.objectValue }

    public static func parse(_ value: EngineValue) -> Result<EngineInfo, EngineInfoProblem> {
        guard case .object(let fields) = value else { return .failure(.notObject) }
        func text(_ key: String) -> String? {
            guard case .string(let text)? = fields[key], !text.isEmpty else { return nil }
            return text
        }
        func list(_ key: String) -> [String]? {
            guard case .array(let items)? = fields[key] else { return nil }
            let strings = items.compactMap(\.stringValue)
            return strings.count == items.count ? strings : nil
        }
        guard let contract = text("contract") else { return .failure(.missingContract) }
        guard let proto = text("protocol") else { return .failure(.missingProtocol) }
        guard let engine = text("engine") else { return .failure(.missingEngine) }
        guard let version = text("version") else { return .failure(.missingVersion) }
        guard contract == EngineInfo.contract else { return .failure(.otherContract) }
        guard isProtocol(proto) else { return .failure(.badProtocol) }
        guard let capabilities = list("capabilities") else { return .failure(.missingCapabilities) }
        guard let ported = list("ported") else { return .failure(.missingPorted) }
        guard let native = list("native") else { return .failure(.missingNative) }
        guard Set(ported).isDisjoint(with: native) else { return .failure(.portedAndNativeOverlap) }
        return .success(EngineInfo(contract: contract, protocol: proto, engine: engine, version: version,
                                   capabilities: capabilities, ported: ported, native: native, fields: fields))
    }

    /// `^[a-z0-9-]+/[0-9]+$`.
    static func isProtocol(_ text: String) -> Bool {
        let parts = text.split(separator: "/", omittingEmptySubsequences: false)
        guard parts.count == 2, !parts[0].isEmpty, !parts[1].isEmpty else { return false }
        return parts[0].utf8.allSatisfy { (0x61...0x7A).contains($0) || (0x30...0x39).contains($0) || $0 == 0x2D }
            && parts[1].utf8.allSatisfy { (0x30...0x39).contains($0) }
    }
}
