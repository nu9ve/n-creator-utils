// Live reading: one message at a time into what the hand did. The crate leaves this to each
// adapter (README: "the adapter keeps the last MSB per control"); here it is shared.

import Foundation

public enum MIDIGesture: Equatable, Sendable {
    case press
    case release
    /// An absolute control, 0–1, already through the dead band.
    case value(Double)
    /// A relative control, in steps with sign.
    case step(Int)
}

public struct MIDIReading: Equatable, Sendable {
    /// The primary address of the control (the MSB for a 14-bit pair).
    public var key: MIDIKey
    public var control: String?
    public var gesture: MIDIGesture
}

public struct MIDIReader: Sendable {
    public let profile: MIDIProfile?
    private var msb: [MIDIKey: UInt16] = [:]
    private var gates: [MIDIKey: MIDIGate] = [:]
    private var down = Set<MIDIKey>()
    /// The dead band when neither the mapping nor the profile gives one (README precedence).
    public static func defaultDeadband(bits: UInt8) -> Double { bits >= 14 ? 0.002 : 0 }

    public init(profile: MIDIProfile?) { self.profile = profile }

    public mutating func read(_ message: MIDIMessage, atMS: UInt64) -> [MIDIReading] {
        let key = message.key
        // The low half of a 14-bit control completes the last MSB; one without an MSB is dropped.
        if key.kind == .cc, let control = profile?.controls.first(where: { $0.lsb == key.number && $0.key.channel == key.channel && $0.kind == .cc }) {
            let primary = profile!.primary(control)
            guard let high = msb[control.key] else { return [] }
            let x = MIDIValue.normalize(MIDIValue.combineCC14(high, message.value), bits: 14)
            return gated(primary.key, primary.id, x, atMS, control.deadband ?? Self.defaultDeadband(bits: 14))
        }
        let control = profile?.control(key: key)
        let id = control.map { profile!.primary($0).id }
        let primaryKey = control.map { profile!.primary($0).key } ?? key
        switch (control?.mode, key.kind) {
        case (.relative(let encoding)?, _):
            let delta = encoding.delta(message.value)
            return delta == 0 ? [] : [MIDIReading(key: primaryKey, control: id, gesture: .step(delta))]
        case (.absolute?, _) where control?.lsb != nil:
            msb[key] = message.value
            return []
        case (.absolute?, .pitchBend), (nil, .pitchBend):
            return gated(primaryKey, id, MIDIValue.normalize(message.value, bits: 14), atMS, control?.deadband ?? Self.defaultDeadband(bits: 14))
        case (.absolute?, _):
            return gated(primaryKey, id, MIDIValue.normalize(message.value, bits: 7), atMS, control?.deadband ?? 0)
        case (nil, .cc):
            // Without a profile a CC may be a button or a knob: report both readings.
            return edge(primaryKey, id, message.pressed) + gated(primaryKey, id, MIDIValue.normalize(message.value, bits: 7), atMS, 0)
        default:
            return edge(primaryKey, id, message.pressed)
        }
    }

    private mutating func edge(_ key: MIDIKey, _ id: String?, _ pressed: Bool) -> [MIDIReading] {
        if pressed, down.insert(key).inserted { return [MIDIReading(key: key, control: id, gesture: .press)] }
        if !pressed, down.remove(key) != nil { return [MIDIReading(key: key, control: id, gesture: .release)] }
        return []
    }

    private mutating func gated(_ key: MIDIKey, _ id: String?, _ x: Double, _ atMS: UInt64, _ deadband: Double) -> [MIDIReading] {
        var gate = gates[key] ?? MIDIGate()
        defer { gates[key] = gate }
        return gate.pass(x, nowMS: atMS, deadband: deadband) ? [MIDIReading(key: key, control: id, gesture: .value(x))] : []
    }
}
