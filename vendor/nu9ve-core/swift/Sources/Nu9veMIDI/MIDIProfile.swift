// Port of nu9ve-midi src/profile.rs (see MIDIMessage.swift), plus profile v2: `feedback` (how each
// control lights up), `grid` and `layout` (contracts/nu9ve-controller-map-v1.md §1). A v1 profile has no output.

import Foundation

public struct MIDIControl: Codable, Equatable, Sendable {
    /// Stable even if the label or wire address is corrected.
    public var id: String
    public var label: String
    public var kind: MIDIKind
    /// 1–16, as manuals write it.
    public var channel: UInt8
    public var number: UInt8
    public var lsb: UInt8?
    public var mode: MIDIMode
    /// Minimum drift from the last accepted value, as a fraction of normalized travel.
    public var deadband: Double?
    /// This address was observed on hardware.
    public var verified: Bool
    public var identified: Bool
    public var evidence: String?
    /// ID of the primary control when this address repeats its output.
    public var mirrorOf: String?
    /// v2: false when the control has no light. Defaults to the profile's `feedback.controls`.
    public var lit: Bool?
    /// v2: where the control sits on the controller's face, for drawing it (column, row from the top-left).
    public var grid: [Int]?

    /// Convert to the runtime key, which uses zero-based channels.
    public var key: MIDIKey { MIDIKey(kind: kind, channel: channel > 0 ? channel - 1 : 0, number: number) }

    enum Keys: String, CodingKey {
        case id, label, kind, channel, number, lsb, mode, deadband, verified, identified, evidence, mirrorOf, lit, grid
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        id = try c.decode(String.self, forKey: .id)
        label = try c.decode(String.self, forKey: .label)
        kind = try c.decode(MIDIKind.self, forKey: .kind)
        channel = try c.decode(UInt8.self, forKey: .channel)
        number = try Self.midiNumber(c, .number)
        lsb = try c.contains(.lsb) && !c.decodeNil(forKey: .lsb) ? Self.midiNumber(c, .lsb) : nil
        mode = try c.decode(MIDIMode.self, forKey: .mode)
        deadband = try c.decodeIfPresent(Double.self, forKey: .deadband)
        verified = try c.decodeIfPresent(Bool.self, forKey: .verified) ?? false
        identified = try c.decodeIfPresent(Bool.self, forKey: .identified) ?? true
        evidence = try c.decodeIfPresent(String.self, forKey: .evidence)
        mirrorOf = try c.decodeIfPresent(String.self, forKey: .mirrorOf)
        lit = try c.decodeIfPresent(Bool.self, forKey: .lit)
        grid = try c.decodeIfPresent([Int].self, forKey: .grid)
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        try c.encode(id, forKey: .id); try c.encode(label, forKey: .label); try c.encode(kind, forKey: .kind)
        try c.encode(channel, forKey: .channel); try c.encode(number, forKey: .number)
        try c.encodeIfPresent(lsb, forKey: .lsb); try c.encode(mode, forKey: .mode)
        try c.encodeIfPresent(deadband, forKey: .deadband); try c.encode(verified, forKey: .verified)
        try c.encode(identified, forKey: .identified); try c.encodeIfPresent(evidence, forKey: .evidence)
        try c.encodeIfPresent(mirrorOf, forKey: .mirrorOf); try c.encodeIfPresent(lit, forKey: .lit)
        try c.encodeIfPresent(grid, forKey: .grid)
    }

    /// Decimal integers or hexadecimal strings ("0x33"), never above 0x7F.
    static func midiNumber(_ c: KeyedDecodingContainer<Keys>, _ key: Keys) throws -> UInt8 {
        if let n = try? c.decode(Int.self, forKey: key) {
            guard (0...0x7f).contains(n) else { throw DecodingError.dataCorruptedError(forKey: key, in: c, debugDescription: "MIDI numbers end at 0x7F: \(n)") }
            return UInt8(n)
        }
        let text = try c.decode(String.self, forKey: key).trimmingCharacters(in: .whitespaces)
        let hex = text.hasPrefix("0x") || text.hasPrefix("0X") ? String(text.dropFirst(2)) : nil
        guard let n = hex.map({ Int($0, radix: 16) }) ?? Int(text), (0...0x7f).contains(n) else {
            throw DecodingError.dataCorruptedError(forKey: key, in: c, debugDescription: "not a MIDI number: \(text)")
        }
        return UInt8(n)
    }
}

public struct MIDIProfile: Codable, Equatable, Sendable {
    public var schemaVersion: Int
    public var id: String
    public var name: String
    public var vendor: String
    /// Substrings matched against a native port name, ignoring case.
    public var ports: [String]
    public var evidence: String
    public var notes: [String]?
    public var controls: [MIDIControl]
    /// v2 only.
    public var feedback: MIDIFeedback?
    /// v2: columns and rows of the controller's face, for `MIDIControl.grid`.
    public var layout: [Int]?
    /// v2: the control the suite reserves to change page on this model (nu9ve-surface-v1 §5).
    public var pageControl: String?

    public static func fromJSON(_ data: Data) throws -> MIDIProfile {
        let profile = try JSONDecoder().decode(MIDIProfile.self, from: data)
        try profile.validate()
        return profile
    }

    /// Profiles that ship with this module (`data/midi/`, embedded like the crate's `include_str!`).
    public static func bundled() -> [MIDIProfile] {
        BundledProfiles.all.compactMap { try? fromJSON(Data($0.json.utf8)) }
    }

    /// The text of a bundled profile, by file name without `.json`.
    public static func bundledText(_ name: String) -> String? { BundledProfiles.all.first { $0.name == name }?.json }

    public func matches(port: String) -> Bool {
        let port = port.lowercased()
        return ports.contains { port.contains($0.lowercased()) }
    }

    public func control(_ id: String) -> MIDIControl? { controls.first { $0.id == id } }

    /// Find a control by its primary address.
    public func control(key: MIDIKey) -> MIDIControl? { controls.first { $0.key == key } }

    /// The primary control for a mirror; the control itself otherwise.
    public func primary(_ control: MIDIControl) -> MIDIControl { control.mirrorOf.flatMap(self.control) ?? control }

    /// Whether the control has a light this profile can drive.
    public func isLit(_ control: MIDIControl) -> Bool {
        guard let feedback, control.mirrorOf == nil else { return false }
        return control.lit ?? feedback.allControls
    }

    public struct Invalid: Error, CustomStringConvertible { public let description: String }

    public func validate() throws {
        guard schemaVersion == 1 || schemaVersion == 2 else { throw Invalid(description: "unsupported MIDI profile version \(schemaVersion)") }
        if schemaVersion == 1, feedback != nil { throw Invalid(description: "\(id): feedback needs profile version 2") }
        guard Self.validID(id), !ports.isEmpty, !ports.contains(where: { $0.trimmingCharacters(in: .whitespaces).isEmpty }) else {
            throw Invalid(description: "\(id): expected a profile ID and nonempty port aliases")
        }
        var ids = Set<String>(), keys = Set<MIDIKey>()
        for control in controls {
            func error(_ reason: String) -> Invalid { Invalid(description: "\(id) · \(control.id): \(reason)") }
            guard Self.validID(control.id), ids.insert(control.id).inserted else { throw error("invalid or duplicate control ID") }
            guard (1...16).contains(control.channel), control.number <= 0x7f else { throw error("invalid MIDI address") }
            if control.kind == .pitchBend && control.number != 0 { throw error("pitch bend must use number 0") }
            guard keys.insert(control.key).inserted else { throw error("duplicate primary address") }
            if let lsb = control.lsb {
                guard control.kind == .cc, lsb <= 0x7f else { throw error("LSB must be a CC numbered 0–127") }
                var low = control.key; low.number = lsb
                guard keys.insert(low).inserted else { throw error("duplicate LSB address") }
            }
            if let deadband = control.deadband, control.mode != .absolute || !(0...1).contains(deadband) {
                throw error("deadband must be between 0 and 1 for an absolute control")
            }
            if let primary = control.mirrorOf {
                guard let found = self.control(primary), found.id != control.id, found.mirrorOf == nil else {
                    throw error("mirror must refer to another primary control")
                }
            }
        }
        try feedback?.validate(profile: id)
        if let pageControl, schemaVersion != 2 || control(pageControl).map({ $0.mirrorOf != nil }) ?? true {
            throw Invalid(description: "\(id): pageControl must be a primary control of a version 2 profile")
        }
    }

    static func validID(_ id: String) -> Bool {
        id.split(separator: "-", omittingEmptySubsequences: false).allSatisfy { part in
            !part.isEmpty && part.allSatisfy { $0.isASCII && ($0.isLowercase || $0.isNumber) }
        }
    }
}
