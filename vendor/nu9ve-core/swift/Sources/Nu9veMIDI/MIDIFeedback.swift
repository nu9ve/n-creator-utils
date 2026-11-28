// Output to a controller's lights: profile v2 `feedback` (contracts/nu9ve-controller-map-v1.md §1). The app asks for a light
// by meaning (a color, dim or bright, still or moving) and the profile says which bytes draw it.

import Foundation

/// Colors an app can ask for. A profile maps each to the closest value it has.
public enum MIDIColor: String, Codable, CaseIterable, Sendable {
    case red, orange, yellow, green, cyan, blue, purple, pink, white
}

public struct MIDILight: Hashable, Sendable {
    public enum Motion: String, Codable, Sendable { case solid, flash, pulse }
    /// nil is off.
    public var color: MIDIColor?
    public var dim: Bool
    public var motion: Motion

    public init(_ color: MIDIColor?, dim: Bool = false, motion: Motion = .solid) {
        self.color = color; self.dim = dim; self.motion = color == nil ? .solid : motion
    }
    public static let off = MIDILight(nil)
    public var isOff: Bool { color == nil }

    /// The default light of an action's state (nu9ve-surface-v1 §3): off dark, available dim, active
    /// bright and warning flashing orange. `color` is the action's or the mapping's.
    public static func forState(_ state: MIDIActionState, color: MIDIColor) -> MIDILight {
        switch state {
        case .off: .off
        case .available: MIDILight(color, dim: true)
        case .active: MIDILight(color)
        case .warning: MIDILight(.orange, motion: .flash)
        }
    }
}

/// The state of an action of an app's catalog (nu9ve-surface-v1 §3).
public enum MIDIActionState: String, Codable, CaseIterable, Sendable {
    case off, available, active, warning
}

public struct MIDIFeedback: Codable, Equatable, Sendable {
    public enum Kind: String, Codable, Sendable {
        /// The value is a color from `palette` (Launchpad, APC).
        case palette
        /// One color: any light is `on`, off is 0 (DJ controllers).
        case onOff
    }
    public var kind: Kind
    /// Every control lights unless it says `"lit": false`; otherwise only those with `"lit": true`.
    public var allControls: Bool
    /// Hex messages sent when the device connects, for example the layout that the addresses assume.
    public var initialize: [String]?
    /// `"red"` and `"red.dim"`; a missing dim falls back to the bright one.
    public var palette: [String: UInt8]?
    public var on: UInt8?
    /// Templates in hex with `{number}` and `{value}`. Without them the app blinks in software.
    public var flash: String?
    public var pulse: String?
    public var evidence: String?

    private enum CodingKeys: String, CodingKey {
        case kind, allControls, initialize = "init", palette, on, flash, pulse, evidence
    }

    public func validate(profile: String) throws {
        if kind == .palette, palette?.isEmpty != false {
            throw MIDIProfile.Invalid(description: "\(profile): a palette feedback needs colors")
        }
        for text in (initialize ?? []) + [flash, pulse].compactMap({ $0 }) {
            let filled = text.replacingOccurrences(of: "{number}", with: "00").replacingOccurrences(of: "{value}", with: "00")
            guard Self.bytes(filled) != nil else { throw MIDIProfile.Invalid(description: "\(profile): not hex bytes: \(text)") }
        }
        if palette?.values.contains(where: { $0 > 0x7f }) == true || (on ?? 0) > 0x7f {
            throw MIDIProfile.Invalid(description: "\(profile): light values end at 0x7F")
        }
    }

    public var initMessages: [[UInt8]] { (initialize ?? []).compactMap(Self.bytes) }

    /// Whether this light needs the app to switch it on and off over time.
    public func blinksInSoftware(_ light: MIDILight) -> Bool {
        switch light.motion {
        case .solid: false
        case .flash: flash == nil
        case .pulse: pulse == nil
        }
    }

    /// The value that draws a still light.
    public func value(_ light: MIDILight) -> UInt8 {
        guard let color = light.color else { return 0 }
        switch kind {
        case .onOff: return on ?? 0x7f
        case .palette:
            let palette = palette ?? [:]
            return (light.dim ? palette["\(color.rawValue).dim"] : nil) ?? palette[color.rawValue] ?? palette["white"] ?? 0x7f
        }
    }

    /// The messages that draw `light` on `control`. `phase` is the software blink: on or off.
    public func messages(_ control: MIDIControl, _ light: MIDILight, phase: Bool = true) -> [[UInt8]] {
        let key = control.key
        let status = (key.kind == .cc ? 0xb0 : 0x90) | key.channel
        let still = [status, key.number, value(light)]
        switch light.motion {
        case .solid: return [still]
        case .flash, .pulse:
            let template = light.motion == .flash ? flash : pulse
            guard let template else { return [phase ? still : [status, key.number, 0]] }
            let filled = template.replacingOccurrences(of: "{number}", with: String(format: "%02X", key.number))
                .replacingOccurrences(of: "{value}", with: String(format: "%02X", value(light)))
            return Self.bytes(filled).map { [still, $0] } ?? [still]
        }
    }

    static func bytes(_ text: String) -> [UInt8]? {
        var out: [UInt8] = []
        for part in text.split(whereSeparator: \.isWhitespace) {
            guard part.count <= 2, let byte = UInt8(part, radix: 16) else { return nil }
            out.append(byte)
        }
        return out.isEmpty ? nil : out
    }
}

/// What the controller shows now. It sends only what changed, everything after a reconnect, and
/// the software blink for profiles that cannot blink by themselves.
public struct MIDILightBoard: Sendable {
    public let profile: MIDIProfile
    private var sent: [String: MIDILight] = [:]
    private var wanted: [String: MIDILight] = [:]

    public init(profile: MIDIProfile) { self.profile = profile }

    /// Messages that take the controller from what was sent to `lights` (control id → light).
    /// Lit controls missing from `lights` are switched off.
    public mutating func update(_ lights: [String: MIDILight]) -> [[UInt8]] {
        guard let feedback = profile.feedback else { return [] }
        var out: [[UInt8]] = []
        for control in profile.controls where profile.isLit(control) {
            let light = lights[control.id] ?? .off
            wanted[control.id] = light
            let before = sent[control.id]
            guard before != light else { continue }
            // Off never needs to be sent to a light that was never lit.
            if light.isOff && before == nil { sent[control.id] = light; continue }
            out += feedback.messages(control, light)
            sent[control.id] = light
        }
        return out
    }

    /// After a (re)connect: the device may show anything, from another app or from before. Every
    /// light goes off, then what is wanted is drawn.
    public mutating func forget() -> [[UInt8]] {
        guard let feedback = profile.feedback else { return [] }
        var out = feedback.initMessages
        sent = [:]
        for control in profile.controls where profile.isLit(control) {
            out += feedback.messages(control, .off)
            sent[control.id] = .off
        }
        let lights = wanted
        return out + update(lights)
    }

    /// The next software blink step, only for lights that need it.
    public func blink(phase: Bool) -> [[UInt8]] {
        guard let feedback = profile.feedback else { return [] }
        return profile.controls.compactMap { control -> [[UInt8]]? in
            guard let light = sent[control.id], feedback.blinksInSoftware(light) else { return nil }
            return feedback.messages(control, light, phase: phase)
        }.flatMap { $0 }
    }

    public var needsBlink: Bool {
        guard let feedback = profile.feedback else { return false }
        return sent.values.contains(where: feedback.blinksInSoftware)
    }

    /// Everything off, when the app lets go of the controller.
    public mutating func darken() -> [[UInt8]] {
        guard let feedback = profile.feedback else { return [] }
        let out = profile.controls.filter { sent[$0.id].map { !$0.isOff } ?? false }.flatMap { feedback.messages($0, .off) }
        sent = [:]; wanted = [:]
        return out
    }
}
