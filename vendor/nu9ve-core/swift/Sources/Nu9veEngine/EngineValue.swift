import Foundation

/// A JSON value that keeps what the engine wrote: numbers as their text (so an id beyond 64 bits
/// comes back with the same digits, and `1` is not `1.0`) and object keys in their order.
public enum EngineValue: Sendable, Hashable {
    case null
    case bool(Bool)
    case number(EngineNumber)
    case string(String)
    case array([EngineValue])
    case object(EngineObject)

    public subscript(key: String) -> EngineValue? {
        if case .object(let object) = self { return object[key] }
        return nil
    }

    public subscript(index: Int) -> EngineValue? {
        if case .array(let items) = self, items.indices.contains(index) { return items[index] }
        return nil
    }

    public var stringValue: String? { if case .string(let value) = self { return value }; return nil }
    public var boolValue: Bool? { if case .bool(let value) = self { return value }; return nil }
    public var numberValue: EngineNumber? { if case .number(let value) = self { return value }; return nil }
    public var arrayValue: [EngineValue]? { if case .array(let value) = self { return value }; return nil }
    public var objectValue: EngineObject? { if case .object(let value) = self { return value }; return nil }
    public var isNull: Bool { self == .null }

    /// Compact JSON text, without raw line breaks. Non-ASCII stays as UTF-8.
    public var jsonData: Data {
        var out = [UInt8]()
        EngineValue.write(self, into: &out)
        return Data(out)
    }

    public var jsonText: String { String(decoding: jsonData, as: UTF8.self) }

    /// Strict RFC 8259 text: one value, UTF-8, no `NaN` or `Infinity`, nothing after it.
    public init(parsing data: Data) throws(EngineJSONError) {
        self = try EngineJSONParser.parse(data).value
    }

    public init(parsing text: String) throws(EngineJSONError) {
        try self.init(parsing: Data(text.utf8))
    }

    /// Any `Encodable`, through `JSONEncoder`.
    public init(encoding value: some Encodable, encoder: JSONEncoder = JSONEncoder()) throws {
        try self.init(parsing: encoder.encode(value))
    }

    /// Decodes through `JSONDecoder`. Integers beyond the target type fail there, not here.
    public func decode<T: Decodable>(_ type: T.Type = T.self, decoder: JSONDecoder = JSONDecoder()) throws -> T {
        try decoder.decode(type, from: jsonData)
    }
}

/// A JSON number as written. Two numbers are equal when their text is.
public struct EngineNumber: Sendable, Hashable, CustomStringConvertible {
    public let text: String

    /// `text` must be a JSON number.
    public init?(text: String) {
        guard EngineNumber.isNumber(Array(text.utf8)) else { return nil }
        self.text = text
    }

    public init(_ value: Int64) { text = String(value) }
    public init(_ value: UInt64) { text = String(value) }
    public init(_ value: Int) { text = String(value) }

    /// Nil for NaN and the infinities, which JSON has no text for.
    public init?(_ value: Double) {
        guard value.isFinite else { return nil }
        text = value == value.rounded() && abs(value) < 1e16 ? String(format: "%.1f", value) : "\(value)"
    }

    init(unchecked text: String) { self.text = text }

    /// Written without fraction or exponent: `7`, `-1`, `18446744073709551616`.
    public var isInteger: Bool { !text.contains(where: { $0 == "." || $0 == "e" || $0 == "E" }) }
    public var int64: Int64? { isInteger ? Int64(text) : nil }
    public var uint64: UInt64? { isInteger ? UInt64(text) : nil }
    public var int: Int? { isInteger ? Int(text) : nil }
    public var double: Double? { Double(text) }
    public var description: String { text }

    static func isNumber(_ bytes: [UInt8]) -> Bool {
        var index = 0
        func digit() -> Bool { index < bytes.count && (0x30...0x39).contains(bytes[index]) }
        if index < bytes.count, bytes[index] == 0x2D { index += 1 }
        guard digit() else { return false }
        if bytes[index] == 0x30 { index += 1 } else { while digit() { index += 1 } }
        if index < bytes.count, bytes[index] == 0x2E {
            index += 1
            guard digit() else { return false }
            while digit() { index += 1 }
        }
        if index < bytes.count, bytes[index] == 0x65 || bytes[index] == 0x45 {
            index += 1
            if index < bytes.count, bytes[index] == 0x2B || bytes[index] == 0x2D { index += 1 }
            guard digit() else { return false }
            while digit() { index += 1 }
        }
        return index == bytes.count
    }
}

/// A JSON object with its keys in order. A repeated key keeps its first place and its last value.
/// Equality ignores the order, like JSON's.
public struct EngineObject: Sendable, Hashable, Sequence, ExpressibleByDictionaryLiteral {
    public private(set) var keys: [String] = []
    private var values: [String: EngineValue] = [:]

    public init() {}

    public init(dictionaryLiteral elements: (String, EngineValue)...) {
        for (key, value) in elements { self[key] = value }
    }

    public init(_ pairs: some Sequence<(String, EngineValue)>) {
        for (key, value) in pairs { self[key] = value }
    }

    public subscript(key: String) -> EngineValue? {
        get { values[key] }
        set {
            if let newValue {
                if values.updateValue(newValue, forKey: key) == nil { keys.append(key) }
            } else if values.removeValue(forKey: key) != nil {
                keys.removeAll { $0 == key }
            }
        }
    }

    public var count: Int { keys.count }
    public var isEmpty: Bool { keys.isEmpty }

    public func makeIterator() -> some IteratorProtocol<(key: String, value: EngineValue)> {
        keys.lazy.map { (key: $0, value: values[$0]!) }.makeIterator()
    }

    public static func == (lhs: EngineObject, rhs: EngineObject) -> Bool { lhs.values == rhs.values }
    public func hash(into hasher: inout Hasher) { hasher.combine(values) }
}

extension EngineValue: ExpressibleByNilLiteral, ExpressibleByBooleanLiteral, ExpressibleByIntegerLiteral,
    ExpressibleByStringLiteral, ExpressibleByArrayLiteral, ExpressibleByDictionaryLiteral {
    public init(nilLiteral: ()) { self = .null }
    public init(booleanLiteral value: Bool) { self = .bool(value) }
    public init(integerLiteral value: Int) { self = .number(EngineNumber(value)) }
    public init(stringLiteral value: String) { self = .string(value) }
    public init(arrayLiteral elements: EngineValue...) { self = .array(elements) }
    public init(dictionaryLiteral elements: (String, EngineValue)...) {
        self = .object(EngineObject(elements.map { ($0.0, $0.1) }))
    }
}

public struct EngineJSONError: Error, Sendable, Equatable, CustomStringConvertible {
    public let offset: Int
    public let problem: String
    public var description: String { "JSON: \(problem) at byte \(offset)" }
}

// MARK: - Reading

struct EngineJSONParser {
    /// Nesting deeper than this is rejected, so hostile input cannot exhaust the stack.
    static let maxDepth = 128

    struct Parsed {
        let value: EngineValue
        /// Byte ranges of the values of a top-level object, by key (for `data` without re-encoding).
        let members: [String: Range<Int>]
    }

    static func parse(_ data: Data) throws(EngineJSONError) -> Parsed {
        let bytes = [UInt8](data)
        var parser = EngineJSONParser(bytes: bytes)
        if bytes.starts(with: [0xEF, 0xBB, 0xBF]) { throw parser.fail("byte order mark") }
        parser.skipSpace()
        let value = try parser.value(depth: 0, recordMembers: true)
        parser.skipSpace()
        guard parser.index == bytes.count else { throw parser.fail("text after the value") }
        return Parsed(value: value, members: parser.members)
    }

    let bytes: [UInt8]
    var index = 0
    var members: [String: Range<Int>] = [:]

    init(bytes: [UInt8]) { self.bytes = bytes }

    func fail(_ problem: String) -> EngineJSONError { EngineJSONError(offset: index, problem: problem) }

    mutating func skipSpace() {
        while index < bytes.count, [0x20, 0x09, 0x0A, 0x0D].contains(bytes[index]) { index += 1 }
    }

    mutating func literal(_ word: StaticString, _ value: EngineValue) throws(EngineJSONError) -> EngineValue {
        let expected = UnsafeBufferPointer(start: word.utf8Start, count: word.utf8CodeUnitCount)
        guard index + expected.count <= bytes.count, bytes[index..<index + expected.count].elementsEqual(expected) else {
            throw fail("unexpected character")
        }
        index += expected.count
        return value
    }

    mutating func value(depth: Int, recordMembers: Bool = false) throws(EngineJSONError) -> EngineValue {
        guard depth <= EngineJSONParser.maxDepth else { throw fail("nesting too deep") }
        guard index < bytes.count else { throw fail("unexpected end") }
        switch bytes[index] {
        case 0x7B: return try object(depth: depth, recordMembers: recordMembers)
        case 0x5B: return try array(depth: depth)
        case 0x22: return .string(try string())
        case 0x74: return try literal("true", .bool(true))
        case 0x66: return try literal("false", .bool(false))
        case 0x6E: return try literal("null", .null)
        case 0x2D, 0x30...0x39: return .number(try number())
        default: throw fail("unexpected character")
        }
    }

    mutating func object(depth: Int, recordMembers: Bool) throws(EngineJSONError) -> EngineValue {
        index += 1
        var object = EngineObject()
        skipSpace()
        if index < bytes.count, bytes[index] == 0x7D { index += 1; return .object(object) }
        while true {
            skipSpace()
            guard index < bytes.count, bytes[index] == 0x22 else { throw fail("expected a key") }
            let key = try string()
            skipSpace()
            guard index < bytes.count, bytes[index] == 0x3A else { throw fail("expected ':'") }
            index += 1
            skipSpace()
            let start = index
            object[key] = try value(depth: depth + 1)
            if recordMembers { members[key] = start..<index }
            skipSpace()
            guard index < bytes.count else { throw fail("unexpected end") }
            if bytes[index] == 0x2C { index += 1; continue }
            if bytes[index] == 0x7D { index += 1; return .object(object) }
            throw fail("expected ',' or '}'")
        }
    }

    mutating func array(depth: Int) throws(EngineJSONError) -> EngineValue {
        index += 1
        var items: [EngineValue] = []
        skipSpace()
        if index < bytes.count, bytes[index] == 0x5D { index += 1; return .array(items) }
        while true {
            skipSpace()
            items.append(try value(depth: depth + 1))
            skipSpace()
            guard index < bytes.count else { throw fail("unexpected end") }
            if bytes[index] == 0x2C { index += 1; continue }
            if bytes[index] == 0x5D { index += 1; return .array(items) }
            throw fail("expected ',' or ']'")
        }
    }

    mutating func number() throws(EngineJSONError) -> EngineNumber {
        let start = index
        while index < bytes.count, [0x2B, 0x2D, 0x2E, 0x45, 0x65].contains(bytes[index]) || (0x30...0x39).contains(bytes[index]) {
            index += 1
        }
        let text = Array(bytes[start..<index])
        guard EngineNumber.isNumber(text) else { index = start; throw fail("invalid number") }
        return EngineNumber(unchecked: String(decoding: text, as: UTF8.self))
    }

    mutating func hex4() throws(EngineJSONError) -> UInt32 {
        guard index + 4 <= bytes.count else { throw fail("short \\u escape") }
        var value: UInt32 = 0
        for byte in bytes[index..<index + 4] {
            let nibble: UInt8
            switch byte {
            case 0x30...0x39: nibble = byte - 0x30
            case 0x41...0x46: nibble = byte - 0x37
            case 0x61...0x66: nibble = byte - 0x57
            default: throw fail("invalid \\u escape")
            }
            value = value << 4 | UInt32(nibble)
        }
        index += 4
        return value
    }

    mutating func string() throws(EngineJSONError) -> String {
        index += 1
        var out = [UInt8]()
        var runStart = index
        while true {
            guard index < bytes.count else { throw fail("unterminated string") }
            let byte = bytes[index]
            if byte == 0x22 {
                out.append(contentsOf: bytes[runStart..<index])
                index += 1
                break
            }
            if byte < 0x20 { throw fail("raw control character in string") }
            if byte != 0x5C { index += 1; continue }
            out.append(contentsOf: bytes[runStart..<index])
            index += 1
            guard index < bytes.count else { throw fail("unterminated escape") }
            let escape = bytes[index]
            index += 1
            switch escape {
            case 0x22, 0x5C, 0x2F: out.append(escape)
            case 0x62: out.append(0x08)
            case 0x66: out.append(0x0C)
            case 0x6E: out.append(0x0A)
            case 0x72: out.append(0x0D)
            case 0x74: out.append(0x09)
            case 0x75:
                var scalar = try hex4()
                if (0xD800...0xDBFF).contains(scalar), index + 6 <= bytes.count, bytes[index] == 0x5C, bytes[index + 1] == 0x75 {
                    let save = index
                    index += 2
                    let low = try hex4()
                    if (0xDC00...0xDFFF).contains(low) {
                        scalar = 0x10000 + (scalar - 0xD800) << 10 + (low - 0xDC00)
                    } else {
                        index = save
                    }
                }
                // A lone surrogate has no Swift string form; it reads as U+FFFD.
                out.append(contentsOf: String(Character(Unicode.Scalar(scalar) ?? "\u{FFFD}")).utf8)
            default:
                throw fail("invalid escape")
            }
            runStart = index
        }
        guard let text = String(validating: out, as: UTF8.self) else { throw fail("invalid UTF-8") }
        return text
    }
}

// MARK: - Writing

extension EngineValue {
    static func write(_ value: EngineValue, into out: inout [UInt8]) {
        switch value {
        case .null: out.append(contentsOf: "null".utf8)
        case .bool(let flag): out.append(contentsOf: (flag ? "true" : "false").utf8)
        case .number(let number): out.append(contentsOf: number.text.utf8)
        case .string(let text): writeString(text, into: &out)
        case .array(let items):
            out.append(0x5B)
            for (offset, item) in items.enumerated() {
                if offset > 0 { out.append(0x2C) }
                write(item, into: &out)
            }
            out.append(0x5D)
        case .object(let object):
            out.append(0x7B)
            for (offset, member) in object.enumerated() {
                if offset > 0 { out.append(0x2C) }
                writeString(member.key, into: &out)
                out.append(0x3A)
                write(member.value, into: &out)
            }
            out.append(0x7D)
        }
    }

    static func writeString(_ text: String, into out: inout [UInt8]) {
        let hex = Array("0123456789abcdef".utf8)
        out.append(0x22)
        for byte in text.utf8 {
            switch byte {
            case 0x22: out.append(contentsOf: [0x5C, 0x22])
            case 0x5C: out.append(contentsOf: [0x5C, 0x5C])
            case 0x0A: out.append(contentsOf: [0x5C, 0x6E])
            case 0x0D: out.append(contentsOf: [0x5C, 0x72])
            case 0x09: out.append(contentsOf: [0x5C, 0x74])
            case 0..<0x20: out.append(contentsOf: [0x5C, 0x75, 0x30, 0x30, hex[Int(byte >> 4)], hex[Int(byte & 0xF)]])
            default: out.append(byte)
            }
        }
        out.append(0x22)
    }
}
