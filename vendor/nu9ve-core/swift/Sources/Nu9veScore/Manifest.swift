import Foundation

/// A score manifest: instruments, events, automation, buses and master. Each app names the schemas it
/// writes and the revision its manifests sound with when they do not say (swift/README.md, Nu9veScore).

public struct ScoreError: Error, CustomStringConvertible {
    public let description: String
}

/// How the engine sounds where two apps' engines once differed. A manifest keeps the sound it was composed
/// with: `v1` is the first engine's (kick, reverb, drive, grains and impact as it had them), `v2` the second's.
/// Everything only one of them had is the same in both revisions, and off unless the manifest asks for it.
public enum ScoreRevision: Int, CaseIterable, Sendable {
    case v1 = 1
    case v2 = 2
}

struct Params {
    let raw: [String: Any]
    func f(_ key: String, _ fallback: Float) -> Float {
        if let value = raw[key] as? NSNumber { return value.floatValue }
        return fallback
    }
    func s(_ key: String, _ fallback: String) -> String { raw[key] as? String ?? fallback }
    func list(_ key: String) -> [Float] { (raw[key] as? [NSNumber])?.map { $0.floatValue } ?? [] }
}

struct InstrumentSpec {
    let id: String
    let kind: String
    let stem: String
    let gain: Float
    let pan: Float
    let params: Params
    let sends: [String: Float]
    let chorus: Float
    let crushBits: Float
    let crushRate: Float
    let drive: Float
    let duck: Float
    let carve: Float
    let pump: Float
    let pumpSource: Bool
    let width: Float
    /// Insert EQ around the drive: high-pass before, cabinet low-pass after,
    /// a mid scoop (480 Hz) and presence lift (1.9 kHz), both 0..1.
    let insertHighpass: Float
    let insertLowpass: Float
    let scoop: Float
    let presence: Float
    let revision: ScoreRevision
}

struct EventSpec {
    let start: Double
    let duration: Double
    let instrument: Int
    let note: Float
    let velocity: Float
    let pan: Float
    let glide: Float
    let index: Int
}

struct AutomationSpec {
    let instrument: Int
    let parameter: String   // cutoff | gain | send:<bus>
    var lane: Lane
}

struct DelaySegment {
    let time: Double
    let left: Float
    let right: Float
    let feedback: Float
    let tone: Float
}

struct BusSpec {
    let rt60: Float
    let size: Float
    let damp: Float
    let predelay: Float
    let gainDb: Float
}

public struct ScoreManifest {
    public let schema: String
    /// From `engine_revision` in the manifest, or the app's default.
    public let revision: ScoreRevision
    let sampleRate: Float
    public let duration: Double
    let seed: UInt64
    let instruments: [InstrumentSpec]
    let events: [EventSpec]
    let automation: [AutomationSpec]
    let duck: RateLane?
    let duckDepthDb: Float
    /// Cue role levels (dB over master time); nil keeps the render without them.
    let level: RateLane?
    let pumpRelease: Float
    let delaySegments: [DelaySegment]
    let buses: [String: BusSpec]
    let masterGainDb: Float
    let glueThresholdDb: Float
    let glueRatio: Float
    let ceilingDb: Float
    let tiltLowDb: Float
    let tiltHighDb: Float
    /// Music-bus openness 0..1 (tension filter on everything but drums/FX).
    let tension: Lane?
    let softClip: Float
    /// Drum-bus clip level (linear, 0 = off): rounds kick/snare peaks before the mix.
    let drumClip: Float
    /// Tape stage 0..1: even-harmonic saturation, high-frequency loss, wow and flutter.
    let tape: Float

    static let stems = ["drums", "bass", "harmony", "lead", "texture", "fx", "space"]

    /// `schemas` are the `schema` values the app accepts; `defaultRevision` is how its manifests sound when
    /// they carry no `engine_revision`.
    public static func load(path: String, schemas: Set<String>, defaultRevision: ScoreRevision) throws -> ScoreManifest {
        try parse(Data(contentsOf: URL(fileURLWithPath: path)), schemas: schemas, defaultRevision: defaultRevision)
    }

    public static func parse(_ data: Data, schemas: Set<String>, defaultRevision: ScoreRevision) throws -> ScoreManifest {
        guard let root = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw ScoreError(description: "manifest is not a JSON object")
        }
        guard let schema = root["schema"] as? String, schemas.contains(schema) else {
            throw ScoreError(description: "unsupported manifest schema \(root["schema"] ?? "nil")")
        }
        var revision = defaultRevision
        if let value = root["engine_revision"] {
            guard let number = value as? NSNumber, let known = ScoreRevision(rawValue: number.intValue), Double(known.rawValue) == number.doubleValue else {
                throw ScoreError(description: "unknown engine_revision \(value)")
            }
            revision = known
        }
        func num(_ dict: [String: Any], _ key: String, _ fallback: Double) -> Double {
            (dict[key] as? NSNumber)?.doubleValue ?? fallback
        }
        let instruments: [InstrumentSpec] = try ((root["instruments"] as? [[String: Any]]) ?? []).map { item in
            guard let kind = item["kind"] as? String else { throw ScoreError(description: "instrument without kind") }
            let insert = item["insert"] as? [String: Any] ?? [:]
            var sends: [String: Float] = [:]
            for (bus, value) in item["sends"] as? [String: Any] ?? [:] { sends[bus] = (value as? NSNumber)?.floatValue ?? 0 }
            let stem = item["stem"] as? String ?? "harmony"
            guard ScoreManifest.stems.contains(stem) else { throw ScoreError(description: "unknown stem \(stem)") }
            return InstrumentSpec(
                id: item["id"] as? String ?? kind, kind: kind, stem: stem,
                gain: dbToGain(Float(num(item, "gain_db", 0))), pan: Float(num(item, "pan", 0)),
                params: Params(raw: item["params"] as? [String: Any] ?? [:]), sends: sends,
                chorus: Float(num(insert, "chorus", 0)), crushBits: Float(num(insert, "crush_bits", 0)),
                crushRate: Float(num(insert, "crush_rate", 0)), drive: Float(num(insert, "drive", 0)),
                duck: Float(num(item, "duck", 0.6)), carve: Float(num(item, "carve", 0)),
                pump: Float(num(item, "pump", 0)), pumpSource: (item["pump_source"] as? Bool) ?? false,
                width: Float(num(item, "width", 1)),
                insertHighpass: Float(num(insert, "highpass", 0)), insertLowpass: Float(num(insert, "lowpass", 0)),
                scoop: Float(num(insert, "scoop", 0)), presence: Float(num(insert, "presence", 0)), revision: revision)
        }
        var events: [EventSpec] = []
        for (index, raw) in ((root["events"] as? [[Any]]) ?? []).enumerated() {
            guard raw.count >= 6 else { throw ScoreError(description: "event \(index) needs 6+ fields") }
            let values = raw.map { ($0 as? NSNumber)?.doubleValue ?? 0 }
            let instrument = Int(values[2])
            guard instrument >= 0 && instrument < instruments.count else {
                throw ScoreError(description: "event \(index) references instrument \(instrument)")
            }
            events.append(EventSpec(start: values[0], duration: max(0.001, values[1]), instrument: instrument,
                                    note: Float(values[3]), velocity: Float(min(1, max(0, values[4]))),
                                    pan: Float(values[5]), glide: raw.count > 6 ? Float(values[6]) : 0, index: index))
        }
        events.sort { $0.start == $1.start ? $0.index < $1.index : $0.start < $1.start }
        let automation: [AutomationSpec] = ((root["automation"] as? [[String: Any]]) ?? []).compactMap { item in
            guard let instrument = (item["inst"] as? NSNumber)?.intValue, instrument < instruments.count,
                  let parameter = item["param"] as? String,
                  let times = item["t"] as? [NSNumber], let values = item["v"] as? [NSNumber], times.count == values.count,
                  !times.isEmpty else { return nil }
            return AutomationSpec(instrument: instrument, parameter: parameter,
                                  lane: Lane(times: times.map { $0.floatValue }, values: values.map { $0.floatValue }))
        }
        var duck: RateLane?
        var duckDepth: Float = 9
        if let block = root["duck"] as? [String: Any], let values = block["values"] as? [NSNumber] {
            duck = RateLane(rate: Float(num(block, "rate", 20)), values: values.map { $0.floatValue })
            duckDepth = Float(num(block, "depth_db", 9))
        }
        let delaySegments = ((root["delay_segments"] as? [[NSNumber]]) ?? []).compactMap { row -> DelaySegment? in
            guard row.count >= 4 else { return nil }
            return DelaySegment(time: row[0].doubleValue, left: row[1].floatValue, right: row[2].floatValue,
                                feedback: row[3].floatValue, tone: row.count > 4 ? row[4].floatValue : 3800)
        }.sorted { $0.time < $1.time }
        var buses: [String: BusSpec] = [
            "room": BusSpec(rt60: 0.7, size: 0.45, damp: 6500, predelay: 0.006, gainDb: 0),
            "hall": BusSpec(rt60: 2.8, size: 1.0, damp: 5200, predelay: 0.022, gainDb: 0),
            "space": BusSpec(rt60: 7.5, size: 1.6, damp: 4200, predelay: 0.045, gainDb: 0),
        ]
        for (name, value) in root["buses"] as? [String: [String: Any]] ?? [:] {
            let fallback = buses[name] ?? buses["hall"]!
            buses[name] = BusSpec(rt60: Float(num(value, "rt60", Double(fallback.rt60))), size: Float(num(value, "size", Double(fallback.size))),
                                  damp: Float(num(value, "damp", Double(fallback.damp))), predelay: Float(num(value, "predelay", Double(fallback.predelay))),
                                  gainDb: Float(num(value, "gain_db", Double(fallback.gainDb))))
        }
        var level: RateLane?
        if let block = root["level"] as? [String: Any], let values = block["values_db"] as? [NSNumber], !values.isEmpty {
            level = RateLane(rate: Float(num(block, "rate", 20)), values: values.map { $0.floatValue })
        }
        let master = root["master"] as? [String: Any] ?? [:]
        var tension: Lane?
        if let block = root["tension"] as? [String: Any], let times = block["t"] as? [NSNumber], let values = block["v"] as? [NSNumber],
           times.count == values.count, !times.isEmpty {
            tension = Lane(times: times.map { $0.floatValue }, values: values.map { $0.floatValue })
        }
        return ScoreManifest(
            schema: schema, revision: revision, sampleRate: Float(num(root, "sample_rate", 48000)), duration: num(root, "duration", 0),
            seed: UInt64(num(root, "seed", 0)), instruments: instruments, events: events, automation: automation,
            duck: duck, duckDepthDb: duckDepth, level: level, pumpRelease: Float(num(root, "pump_release", 0.22)),
            delaySegments: delaySegments, buses: buses, masterGainDb: Float(num(master, "gain_db", 0)),
            glueThresholdDb: Float(num(master, "glue_threshold_db", -18)), glueRatio: Float(num(master, "glue_ratio", 1.8)),
            ceilingDb: Float(num(master, "ceiling_db", -1)), tiltLowDb: Float(num(master, "low_shelf_db", 0)),
            tiltHighDb: Float(num(master, "high_shelf_db", 0)), tension: tension,
            softClip: Float(num(master, "soft_clip", 0)), drumClip: Float(num(master, "drum_clip", 0)), tape: Float(num(master, "tape", 0)))
    }
}
