// Provisional port of nu9ve-midi src/input.rs (see MIDIMessage.swift).

import Foundation

/// How a relative encoder (jog, endless knob, browse) writes a step into 7 bits.
public enum MIDIEncoding: String, Codable, Sendable {
    /// 64 is still, 65 is +1, 63 is −1. Pioneer and Numark jogs.
    case offset64
    /// 1 is +1, 127 is −1. Pioneer's browse knob, many endless knobs.
    case twosComplement
    /// Bit 6 is the sign: 1 is +1, 65 is −1.
    case signBit

    public func delta(_ value: UInt16) -> Int {
        let v = Int(value & 0x7f)
        switch self {
        case .offset64: return v - 64
        case .twosComplement: return v >= 64 ? v - 128 : v
        case .signBit: return v & 0x40 != 0 ? -(v & 0x3f) : v & 0x3f
        }
    }
}

/// How a bound control is read. Serialized as the crate does: `{"type":"relative","encoding":"offset64"}`.
public enum MIDIMode: Codable, Hashable, Sendable {
    /// Legacy bindings without an explicit mode.
    case auto
    /// Pressed and released, by note or by CC.
    case button
    /// A fader or knob with ends: 7-bit, or 14-bit with an LSB.
    case absolute
    /// An endless encoder sending steps.
    case relative(MIDIEncoding)

    private enum Keys: String, CodingKey { case type, encoding }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        switch try c.decode(String.self, forKey: .type) {
        case "auto": self = .auto
        case "button": self = .button
        case "absolute": self = .absolute
        case "relative": self = .relative(try c.decode(MIDIEncoding.self, forKey: .encoding))
        case let other: throw DecodingError.dataCorruptedError(forKey: .type, in: c, debugDescription: "unknown mode \(other)")
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case .auto: try c.encode("auto", forKey: .type)
        case .button: try c.encode("button", forKey: .type)
        case .absolute: try c.encode("absolute", forKey: .type)
        case .relative(let encoding):
            try c.encode("relative", forKey: .type); try c.encode(encoding, forKey: .encoding)
        }
    }
}

public enum MIDIValue {
    /// Join the two 7-bit values from an MSB/LSB pair.
    public static func combineCC14(_ msb: UInt16, _ lsb: UInt16) -> UInt16 { (msb & 0x7f) << 7 | (lsb & 0x7f) }

    /// 0–1 from a raw value, with the centre detent (64 of 127, 8192 of 16383) at exactly 0.5.
    public static func normalize(_ raw: UInt16, bits: UInt8) -> Double {
        let max: Double = bits >= 14 ? 16383 : 127
        let centre = (max + 1) / 2
        let raw = min(Double(raw), max)
        return raw <= centre ? raw / centre * 0.5 : 0.5 + (raw - centre) / (max - centre) * 0.5
    }

    /// Messages closer than this belong to one movement.
    public static let motionMS: UInt64 = 40
}

/// Keeps a worn fader still: a lone message gets through only past `deadband` (of 0–1); a hand
/// moving every few milliseconds gets through at full resolution.
public struct MIDIGate: Sendable {
    private var anchor: Double?
    private var last: UInt64?

    public init() {}

    public mutating func pass(_ x: Double, nowMS: UInt64, deadband: Double) -> Bool {
        let moving = last.map { (nowMS >= $0 ? nowMS - $0 : 0) <= MIDIValue.motionMS } ?? false
        last = nowMS
        let far = anchor.map { abs(x - $0) >= deadband } ?? true
        if moving || far { anchor = x; return true }
        return false
    }
}
