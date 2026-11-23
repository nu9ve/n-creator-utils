import Foundation
import Testing
import Nu9veFormat

// nu9ve-controls-v1 §12: data/ui-format-vectors-v1.json holds what Nu9veFormat gives for fixed inputs, so the copy
// in Nu9ve.Core (C#) and the web check the same. With NU9VE_WRITE_UI_VECTORS=1, `swift test` rewrites it here.

struct FormatVectors: Codable, Equatable {
    struct Spec: Codable, Equatable {
        let id: String
        let min: Double
        let max: Double
        let step: Double
        let defaultValue: Double
        let origin: Double
        let scale: String
        let unit: String
        let fractionDigits: Int
        let largeSteps: Double
    }
    /// One operation on a spec. `op`: position, value, snapped, stepped, text, isDefault.
    struct ParamCase: Codable, Equatable {
        let op: String
        let input: Double
        var kind: String?
        var count: Int?
        var decimalSeparator: String?
        var number: Double?
        var text: String?
        var flag: Bool?
    }
    struct Param: Codable, Equatable {
        let spec: Spec
        let cases: [ParamCase]
    }
    struct NumberCase: Codable, Equatable {
        let value: Double
        let fractionDigits: Int
        let unit: String
        let decimalSeparator: String
        let text: String
    }
    struct BytesCase: Codable, Equatable {
        let bytes: Int64
        let decimalSeparator: String
        let text: String
    }
    struct TimecodeCase: Codable, Equatable {
        let seconds: Double
        var framesPerSecond: Int?
        let text: String
    }
    struct InstantCase: Codable, Equatable {
        let raw: String
        let seconds: Double?
        /// Wall clock by zone identifier, when it is an instant.
        let wallClock: [String: String]?
    }

    let contract: String
    let note: String
    let params: [Param]
    let numbers: [NumberCase]
    let bytes: [BytesCase]
    let timecodes: [TimecodeCase]
    let instants: [InstantCase]
}

private let zones = ["UTC", "America/Hermosillo", "Europe/Madrid"]

private func spec(_ s: ParamSpec) -> FormatVectors.Spec {
    .init(id: s.id, min: s.min, max: s.max, step: s.step, defaultValue: s.defaultValue, origin: s.origin,
          scale: s.scale.rawValue, unit: s.unit, fractionDigits: s.fractionDigits, largeSteps: s.largeSteps)
}

private func cases(for s: ParamSpec, values: [Double], positions: [Double]) -> [FormatVectors.ParamCase] {
    var out: [FormatVectors.ParamCase] = []
    for v in values {
        out.append(.init(op: "position", input: v, number: s.position(of: v)))
        out.append(.init(op: "snapped", input: v, number: s.snapped(v)))
        out.append(.init(op: "isDefault", input: v, flag: s.isDefault(v)))
        for kind in ParamStep.allCases {
            for count in [1, -1] {
                out.append(.init(op: "stepped", input: v, kind: kind.rawValue, count: count, number: s.stepped(v, kind, count: count)))
            }
        }
        for separator in [",", "."] {
            out.append(.init(op: "text", input: v, decimalSeparator: separator, text: s.text(v, decimalSeparator: separator)))
        }
    }
    for p in positions { out.append(.init(op: "value", input: p, number: s.value(at: p))) }
    return out
}

func generatedFormatVectors() -> FormatVectors {
    let gain = ParamSpec(id: "gain", label: "Ganancia", min: -24, max: 24, step: 0.5, defaultValue: 0, origin: 0, unit: "dB", fractionDigits: 1)
    let frequency = ParamSpec(id: "frequency", label: "Frecuencia", min: 20, max: 20_000, step: 0.01, defaultValue: 1000, scale: .log, unit: "Hz")
    let temperature = ParamSpec(id: "temperature", label: "Temperatura", min: 2000, max: 50_000, step: 0.01, defaultValue: 5600, scale: .log, unit: "K")
    let mix = ParamSpec(id: "mix", label: "Mezcla", min: 0, max: 100, step: 1, defaultValue: 50, unit: "%")
    let rate = ParamSpec(id: "rate", label: "Velocidad", min: 0.25, max: 4, step: 0.05, defaultValue: 1, unit: "×", fractionDigits: 2, largeSteps: 5)
    let params = [
        FormatVectors.Param(spec: spec(gain), cases: cases(for: gain, values: [-24, -6, -0.24, 0, 0.26, 3.75, 24, 30, .nan], positions: [0, 0.25, 0.5, 1, -1, 2])),
        FormatVectors.Param(spec: spec(frequency), cases: cases(for: frequency, values: [20, 440, 1000, 19_999, 20_000, 5], positions: [0, 0.5, 1])),
        FormatVectors.Param(spec: spec(temperature), cases: cases(for: temperature, values: [2000, 5600, 6500, 50_000], positions: [0, 0.3, 1])),
        FormatVectors.Param(spec: spec(mix), cases: cases(for: mix, values: [0, 49.5, 50, 99.6, 100], positions: [0, 0.505, 1])),
        FormatVectors.Param(spec: spec(rate), cases: cases(for: rate, values: [0.25, 1, 1.125, 3.99, 4], positions: [0, 0.2, 1]))
    ]

    let numberInputs: [(Double, Int, String)] = [
        (-6, 1, "dB"), (440, 0, "Hz"), (5600, 0, "K"), (1.25, 2, "×"), (50, 0, "%"), (-0.04, 1, "dB"), (-0.05, 1, "dB"),
        (0.125, 2, ""), (2.5, 0, ""), (-2.5, 0, ""), (1_234_567.891, 3, "m"), (0, 3, "s"), (.infinity, 1, "dB"), (21.5, 1, "°")
    ]
    let numbers = numberInputs.flatMap { value, digits, unit in
        [",", "."].map { FormatVectors.NumberCase(value: value, fractionDigits: digits, unit: unit, decimalSeparator: $0,
                                                   text: NumberText.format(value, fractionDigits: digits, unit: unit, decimalSeparator: $0)) }
    }

    let byteInputs: [Int64] = [0, 1, 999, 1000, 1049, 1050, 9_949, 9_950, 99_499, 999_499, 999_500, 999_950_000, 1_200_000_000, 120_000_000_000, 4_000_000_000_000, -2048]
    let bytes = byteInputs.flatMap { b in [",", "."].map { FormatVectors.BytesCase(bytes: b, decimalSeparator: $0, text: NumberText.bytes(b, decimalSeparator: $0)) } }

    let plain: [Double] = [0, 0.004, 0.005, 1.5, 59.994, 59.995, 61.25, 3599.99, 3600, 3725.5, 36_000.01, -1.5, -0.001, .nan, .infinity]
    var timecodes = plain.map { FormatVectors.TimecodeCase(seconds: $0, text: Timecode.format($0)) }
    let framed: [(Double, Int)] = [(0, 25), (1.0, 25), (1.039, 25), (1.04, 25), (59.999, 30), (3600.5, 24), (10.0, 120), (-2.0, 25), (1, 0), (.nan, 25)]
    timecodes += framed.map { FormatVectors.TimecodeCase(seconds: $0.0, framesPerSecond: $0.1, text: Timecode.format($0.0, framesPerSecond: $0.1)) }

    let raws = [
        "2026-11-22T18:30:00Z", "2026-11-22T18:30:00.123456+00:00", "2026-11-22T11:30-07:00", "2026-11-22 18:30:05Z",
        "2026-11-22T18:30:00+0530", "2024-02-29T23:59:59Z", "2026-03-29T00:59:00Z", "2026-03-29T01:00:00Z",
        "1969-12-31T23:59:59Z", "2000-01-01T00:00:00.5Z",
        "2026-11-22", "2026-11-22T18:30:00", "2025-02-29T00:00:00Z", "2026-13-01T00:00Z", "2026-11-22T24:00Z",
        "2026-11-22T18:30:00.Z", "2026-11-22T18:30:00.1234567890Z", "", "mañana", "2026-11-22T18:30:00Z "
    ]
    let instants = raws.map { raw -> FormatVectors.InstantCase in
        let seconds = Instant.seconds(raw)
        let clocks = seconds.map { s in
            Dictionary(uniqueKeysWithValues: zones.map { ($0, Instant.wallClock(s, timeZone: TimeZone(identifier: $0)!)) })
        }
        return .init(raw: raw, seconds: seconds, wallClock: clocks)
    }

    return FormatVectors(
        contract: "nu9ve-controls-v1 §3 §4",
        note: "Generated by Nu9veFormat (NU9VE_WRITE_UI_VECTORS=1 swift test). Numbers compare with a relative tolerance of 1e-9; text, exactly. Values that are not finite go as the strings NaN, Infinity and -Infinity.",
        params: params, numbers: numbers, bytes: bytes, timecodes: timecodes, instants: instants
    )
}

private let vectorsURL = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent().deletingLastPathComponent()
    .deletingLastPathComponent().deletingLastPathComponent()
    .appendingPathComponent("data/ui-format-vectors-v1.json")

private func close(_ a: Double?, _ b: Double?) -> Bool {
    switch (a, b) {
    case (nil, nil): return true
    case let (x?, y?): return x == y || abs(x - y) <= 1e-9 * max(1, abs(x), abs(y))
    default: return false
    }
}

@Test func formatVectorsMatchThisImplementation() throws {
    let generated = generatedFormatVectors()
    if ProcessInfo.processInfo.environment["NU9VE_WRITE_UI_VECTORS"] == "1" {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        encoder.nonConformingFloatEncodingStrategy = .convertToString(positiveInfinity: "Infinity", negativeInfinity: "-Infinity", nan: "NaN")
        try (encoder.encode(generated) + Data("\n".utf8)).write(to: vectorsURL)
    }
    let decoder = JSONDecoder()
    decoder.nonConformingFloatDecodingStrategy = .convertFromString(positiveInfinity: "Infinity", negativeInfinity: "-Infinity", nan: "NaN")
    let stored = try decoder.decode(FormatVectors.self, from: Data(contentsOf: vectorsURL))
    #expect(stored.contract == generated.contract)
    #expect(stored.params.count == generated.params.count)
    for (want, got) in zip(stored.params, generated.params) {
        #expect(want.spec == got.spec)
        #expect(want.cases.count == got.cases.count, "\(want.spec.id)")
        for (w, g) in zip(want.cases, got.cases) {
            let label = "\(want.spec.id) \(w.op) \(w.input) \(w.kind ?? "") \(w.count ?? 0)"
            #expect(w.op == g.op && (w.input == g.input || (w.input.isNaN && g.input.isNaN)), "\(label)")
            #expect(close(w.number, g.number), "\(label): \(String(describing: g.number))")
            #expect(w.text == g.text && w.flag == g.flag, "\(label): \(g.text ?? "") \(String(describing: g.flag))")
        }
    }
    #expect(stored.numbers == generated.numbers)
    #expect(stored.bytes == generated.bytes)
    #expect(stored.timecodes.count == generated.timecodes.count)
    for (w, g) in zip(stored.timecodes, generated.timecodes) { #expect(w.text == g.text, "\(w.seconds) \(String(describing: w.framesPerSecond))") }
    #expect(stored.instants.count == generated.instants.count)
    for (w, g) in zip(stored.instants, generated.instants) {
        #expect(w.raw == g.raw && close(w.seconds, g.seconds) && w.wallClock == g.wallClock, "\(w.raw)")
    }
}
