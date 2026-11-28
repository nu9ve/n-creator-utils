import Foundation
import Testing
@testable import Nu9veMIDI

// data/midi/controller-vectors-v1.json: the cases nu9ve-midi (Rust) passes too. Swift is the reference:
// `NU9VE_WRITE_MIDI_VECTORS=1 swift test --filter ControllerVectors` writes every `expect` from Nu9veMIDI.

private let vectorsURL = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
    .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("data/midi/controller-vectors-v1.json")

private func hex(_ messages: [[UInt8]]) -> [String] {
    messages.map { $0.map { String(format: "%02X", $0) }.joined(separator: " ") }
}

private func light(_ any: Any?) -> MIDILight {
    let o = any as? [String: Any] ?? [:]
    let color = (o["color"] as? String).flatMap(MIDIColor.init(rawValue:))
    let motion = (o["motion"] as? String).flatMap(MIDILight.Motion.init(rawValue:)) ?? .solid
    return MIDILight(color, dim: o["dim"] as? Bool ?? false, motion: motion)
}

private func lightJSON(_ l: MIDILight) -> [String: Any] {
    ["color": l.color.map { $0.rawValue as Any } ?? NSNull(), "dim": l.dim, "motion": l.motion.rawValue]
}

private func lights(_ any: Any?) -> [String: MIDILight] {
    (any as? [String: Any] ?? [:]).mapValues(light)
}

/// A profile of data/midi/, with the case's patch: top-level fields replaced and `lit` set per control.
private func profile(_ spec: [String: Any]) throws -> MIDIProfile {
    let data = try Data(contentsOf: vectorsURL.deletingLastPathComponent().appendingPathComponent(spec["file"] as! String))
    var json = try JSONSerialization.jsonObject(with: data) as! [String: Any]
    if let patch = spec["patch"] as? [String: Any] {
        for (key, value) in patch where key != "lit" { json[key] = value }
        if let lit = patch["lit"] as? [String: Bool] {
            json["controls"] = (json["controls"] as! [[String: Any]]).map { control in
                var control = control
                if let on = lit[control["id"] as! String] { control["lit"] = on }
                return control
            }
        }
    }
    return try MIDIProfile.fromJSON(JSONSerialization.data(withJSONObject: json))
}

private func readingJSON(_ r: MIDIReading) -> [String: Any] {
    let gesture: [String: Any] = switch r.gesture {
    case .press: ["kind": "press"]
    case .release: ["kind": "release"]
    case .value(let x): ["kind": "value", "value": x]
    case .step(let d): ["kind": "step", "delta": d]
    }
    return ["key": ["kind": r.key.kind.rawValue, "channel": Int(r.key.channel), "number": Int(r.key.number)],
            "control": r.control.map { $0 as Any } ?? NSNull(), "gesture": gesture]
}

private func bytes(_ text: String) -> [UInt8] { text.split(separator: " ").map { UInt8($0, radix: 16)! } }

/// Every case with what Nu9veMIDI gives for it.
private func computed(_ root: [String: Any]) throws -> [String: Any] {
    var root = root
    var profiles: [String: MIDIProfile] = [:]
    for (name, spec) in root["profiles"] as! [String: [String: Any]] { profiles[name] = try profile(spec) }

    root["lit"] = (root["lit"] as! [[String: Any]]).map { c in
        var c = c
        let p = profiles[c["profile"] as! String]!
        c["expect"] = p.isLit(p.control(c["control"] as! String)!)
        return c
    }
    root["lights"] = (root["lights"] as! [[String: Any]]).map { c in
        var c = c
        let p = profiles[c["profile"] as! String]!
        c["expect"] = hex(p.feedback!.messages(p.control(c["control"] as! String)!, light(c["light"]), phase: c["phase"] as! Bool))
        return c
    }
    root["softwareBlink"] = (root["softwareBlink"] as! [[String: Any]]).map { c in
        var c = c
        c["expect"] = profiles[c["profile"] as! String]!.feedback!.blinksInSoftware(light(c["light"]))
        return c
    }
    root["states"] = (root["states"] as! [[String: Any]]).map { c in
        var c = c
        c["expect"] = lightJSON(.forState(MIDIActionState(rawValue: c["state"] as! String)!, color: MIDIColor(rawValue: c["color"] as! String)!))
        return c
    }
    root["boards"] = (root["boards"] as! [[String: Any]]).map { b in
        var b = b
        var board = MIDILightBoard(profile: profiles[b["profile"] as! String]!)
        b["steps"] = (b["steps"] as! [[String: Any]]).map { s in
            var s = s
            if let update = s["update"] { s["expect"] = hex(board.update(lights(update))) }
            else if s["forget"] != nil { s["expect"] = hex(board.forget()) }
            else if s["darken"] != nil { s["expect"] = hex(board.darken()) }
            else if let phase = s["blink"] as? Bool { s["expect"] = hex(board.blink(phase: phase)) }
            else if s["needsBlink"] != nil { s["expect"] = board.needsBlink }
            return s
        }
        return b
    }
    root["readings"] = (root["readings"] as! [[String: Any]]).map { r in
        var r = r
        var reader = MIDIReader(profile: (r["profile"] as? String).flatMap { profiles[$0] })
        r["steps"] = (r["steps"] as! [[String: Any]]).map { s in
            var s = s
            let message = MIDIMessage.parse(bytes(s["bytes"] as! String))
            let at = UInt64(s["at"] as! Int)
            s["expect"] = message.map { reader.read($0, atMS: at).map(readingJSON) } ?? []
            return s
        }
        return r
    }
    return root
}

@Suite struct ControllerVectors {
    @Test func nu9veMIDIGivesEveryExpectation() throws {
        let file = try JSONSerialization.jsonObject(with: Data(contentsOf: vectorsURL)) as! [String: Any]
        let ours = try computed(file)
        if ProcessInfo.processInfo.environment["NU9VE_WRITE_MIDI_VECTORS"] == "1" {
            let data = try JSONSerialization.data(withJSONObject: ours, options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes])
            try (data + Data("\n".utf8)).write(to: vectorsURL)
            return
        }
        // Compared as JSON values, so key order and number spelling do not matter.
        let a = try JSONSerialization.data(withJSONObject: file, options: [.sortedKeys])
        let b = try JSONSerialization.data(withJSONObject: ours, options: [.sortedKeys])
        #expect(a == b, "run NU9VE_WRITE_MIDI_VECTORS=1 swift test --filter ControllerVectors and review the diff")
    }
}
