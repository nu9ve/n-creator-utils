// The controller map of contracts/nu9ve-controller-map-v1.md: control → an action id of the app's
// catalog. Its shape keeps the first preset format of the suite (`{profile, mappings: [{control,
// action}]}`), with the action as a catalog id and an argument instead of an app's own slots.

import Foundation

/// A raw address, with the channel from 1 as in profiles and manuals.
public struct MIDIAddress: Codable, Hashable, Sendable {
    public var kind: MIDIKind
    public var channel: UInt8
    public var number: UInt8

    public init(_ key: MIDIKey) { kind = key.kind; channel = key.channel + 1; number = key.number }
    public var key: MIDIKey { MIDIKey(kind: kind, channel: channel > 0 ? channel - 1 : 0, number: number) }

    private enum Keys: String, CodingKey { case kind, channel, number }
    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        kind = try c.decode(MIDIKind.self, forKey: .kind)
        channel = try c.decode(UInt8.self, forKey: .channel)
        if let n = try? c.decode(UInt8.self, forKey: .number) { number = n } else {
            let text = try c.decode(String.self, forKey: .number)
            guard let n = UInt8(text.replacingOccurrences(of: "0x", with: ""), radix: 16), n <= 0x7f else {
                throw DecodingError.dataCorruptedError(forKey: .number, in: c, debugDescription: "not a MIDI number: \(text)")
            }
            number = n
        }
        guard (1...16).contains(channel), number <= 0x7f else {
            throw DecodingError.dataCorruptedError(forKey: .channel, in: c, debugDescription: "invalid MIDI address")
        }
    }
    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        try c.encode(kind, forKey: .kind); try c.encode(channel, forKey: .channel)
        try c.encode(String(format: "0x%02X", number), forKey: .number)
    }
}

/// Where a mapping points: a profile control, or a raw address when there is no profile.
public enum MIDITarget: Hashable, Sendable {
    case control(String)
    case address(MIDIAddress)
}

public struct MIDIMapping: Codable, Hashable, Sendable {
    public var control: String?
    public var key: MIDIAddress?
    public var action: String
    public var argument: String?
    /// The light's color for this mapping; the app chooses one when nil.
    public var color: MIDIColor?

    public init(_ target: MIDITarget, action: String, argument: String? = nil, color: MIDIColor? = nil) {
        switch target {
        case .control(let id): control = id; key = nil
        case .address(let address): control = nil; key = address
        }
        self.action = action; self.argument = argument; self.color = color
    }

    public var target: MIDITarget? {
        if let control { return .control(control) }
        return key.map(MIDITarget.address)
    }
}

/// Controls that belong to an app even without the page (nu9ve-surface-v1 §5 and §6).
public struct MIDIZone: Codable, Hashable, Sendable {
    public enum When: String, Codable, Sendable { case always, recording }
    public var when: When
    public var controls: [String]
    public init(when: When, controls: [String]) { self.when = when; self.controls = controls }
}

/// `modes.recording.mappings` replaces the mappings of those controls while the suite records (surface §6).
public struct MIDIModes: Codable, Hashable, Sendable {
    public struct Mappings: Codable, Hashable, Sendable { public var mappings: [MIDIMapping] }
    public var recording: Mappings?
}

public struct MIDIDeviceMap: Codable, Hashable, Sendable {
    /// Substring of the port name, ignoring case.
    public var match: String
    public var profile: String?
    public var mappings: [MIDIMapping]
    /// Surface §6, optional.
    public var zones: [MIDIZone]?
    public var modes: MIDIModes?

    public init(match: String, profile: String?, mappings: [MIDIMapping] = []) {
        self.match = match; self.profile = profile; self.mappings = mappings
    }

    public func matches(port: String) -> Bool { port.lowercased().contains(match.lowercased()) }

    /// The mapping a message belongs to: by the profile's control at that address, then by address.
    public func mapping(for key: MIDIKey, profile: MIDIProfile?) -> MIDIMapping? {
        if let control = profile?.control(key: key).map({ profile!.primary($0) }),
           let found = mappings.first(where: { $0.control == control.id }) { return found }
        return mappings.first { $0.key?.key == key }
    }

    public func mappings(action: String, argument: String? = nil) -> [MIDIMapping] {
        mappings.filter { $0.action == action && $0.argument == argument }
    }

    /// Gives `target` this action; one target holds one action. Returns the mapping it displaced.
    @discardableResult
    public mutating func assign(_ target: MIDITarget, action: String, argument: String? = nil, color: MIDIColor? = nil) -> MIDIMapping? {
        let displaced = mappings.first { $0.target == target }
        mappings.removeAll { $0.target == target }
        mappings.append(MIDIMapping(target, action: action, argument: argument, color: color ?? displaced?.color))
        return displaced.flatMap { $0.action == action && $0.argument == argument ? nil : $0 }
    }

    public mutating func remove(action: String, argument: String? = nil) {
        mappings.removeAll { $0.action == action && $0.argument == argument }
    }

    /// Targets used more than once: only a hand-edited file has them. The first one wins.
    public var conflicts: [MIDITarget] {
        var seen = Set<MIDITarget>(), twice: [MIDITarget] = []
        for target in mappings.compactMap(\.target) where !seen.insert(target).inserted && !twice.contains(target) { twice.append(target) }
        return twice
    }
}

public struct MIDIMapFile: Codable, Equatable, Sendable {
    public static let version = 1
    public var schemaVersion: Int
    /// Off: the app does not open CoreMIDI at all.
    public var enabled: Bool
    public var devices: [MIDIDeviceMap]

    public init(enabled: Bool = false, devices: [MIDIDeviceMap] = []) {
        schemaVersion = Self.version; self.enabled = enabled; self.devices = devices
    }

    public static func decode(_ data: Data) throws -> MIDIMapFile {
        let file = try JSONDecoder().decode(MIDIMapFile.self, from: data)
        guard file.schemaVersion == version else { throw MIDIProfile.Invalid(description: "unsupported controller map version \(file.schemaVersion)") }
        return file
    }

    public func encoded() throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(self) + Data("\n".utf8)
    }

    public func device(port: String) -> MIDIDeviceMap? { devices.first { $0.matches(port: port) } }
}
