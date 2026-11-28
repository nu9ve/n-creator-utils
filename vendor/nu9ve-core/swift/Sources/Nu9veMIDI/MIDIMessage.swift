// Nu9veMIDI: a port of `rust/nu9ve-midi` (src/message.rs at b4dee11), line by line, plus what the
// crate does not have yet (output to lights, maps, live reading, CoreMIDI). Same vectors as the crate
// (Tests/Nu9veMIDITests). It knows no app: each passes its own catalog of actions.

import Foundation

/// The supported channel message families. Everything else (clock, SysEx, aftertouch) is ignored.
public enum MIDIKind: String, Codable, Sendable, Comparable, CaseIterable {
    /// Note On and Note Off. A Note On with velocity 0 is a release.
    case note
    /// Control Change, 7 bits, or the MSB of a 14-bit pair.
    case cc
    /// Pitch bend: one 14-bit value per channel.
    case pitchBend

    /// The status nibble stored with a binding.
    public var status: UInt8 {
        switch self {
        case .note: 0x90
        case .cc: 0xb0
        case .pitchBend: 0xe0
        }
    }

    public init?(status: UInt8) {
        switch status & 0xf0 {
        case 0x80, 0x90: self = .note
        case 0xb0: self = .cc
        case 0xe0: self = .pitchBend
        default: return nil
        }
    }

    private var order: Int { Self.allCases.firstIndex(of: self)! }
    public static func < (a: MIDIKind, b: MIDIKind) -> Bool { a.order < b.order }
}

/// One physical control: what it sends, on which channel (0–15) and which number.
public struct MIDIKey: Codable, Hashable, Sendable, Comparable {
    public var kind: MIDIKind
    public var channel: UInt8
    /// Note or CC number; 0 for pitch bend.
    public var number: UInt8

    public init(kind: MIDIKind, channel: UInt8, number: UInt8) {
        self.kind = kind; self.channel = channel; self.number = number
    }

    /// How people read it in a manual: channels from 1, numbers in hex.
    public var label: String {
        switch kind {
        case .note: String(format: "CH %d · NOTE 0x%02X", Int(channel) + 1, Int(number))
        case .cc: String(format: "CH %d · CC 0x%02X", Int(channel) + 1, Int(number))
        case .pitchBend: "CH \(Int(channel) + 1) · PITCH BEND"
        }
    }

    public static func < (a: MIDIKey, b: MIDIKey) -> Bool {
        (a.kind, a.channel, a.number) < (b.kind, b.channel, b.number)
    }
}

public struct MIDIMessage: Equatable, Sendable {
    public var key: MIDIKey
    /// Velocity (0 when released), CC value 0–127, or pitch bend 0–16383.
    public var value: UInt16

    public init(key: MIDIKey, value: UInt16) { self.key = key; self.value = value }

    /// Reads a channel message; anything else is nil.
    public static func parse(_ bytes: [UInt8]) -> MIDIMessage? {
        guard bytes.count >= 3 else { return nil }
        let status = bytes[0], a = bytes[1], b = bytes[2]
        guard a <= 0x7f, b <= 0x7f, let kind = MIDIKind(status: status) else { return nil }
        let channel = status & 0x0f
        let number: UInt8, value: UInt16
        switch status & 0xf0 {
        case 0x80: (number, value) = (a, 0)
        case 0xe0: (number, value) = (0, UInt16(b) << 7 | UInt16(a))
        default: (number, value) = (a, UInt16(b))
        }
        return MIDIMessage(key: MIDIKey(kind: kind, channel: channel, number: number), value: value)
    }

    /// A note held or a CC at or above the middle: how a button reads either.
    public var pressed: Bool {
        switch key.kind {
        case .note: value > 0
        case .cc: value >= 64
        case .pitchBend: false
        }
    }
}
