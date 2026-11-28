// Provisional port of nu9ve-midi src/detect.rs (see MIDIMessage.swift). From a gesture to a
// control: everything a controller sent while one control was moved, ending when it goes quiet.

import Foundation

public struct MIDIEvent: Equatable, Sendable {
    public var atMS: UInt64
    public var message: MIDIMessage
    public init(atMS: UInt64, message: MIDIMessage) { self.atMS = atMS; self.message = message }
}

/// What one gesture was.
public struct MIDIDetected: Equatable, Sendable {
    public var key: MIDIKey
    /// The CC carrying the low 7 bits of a 14-bit control (by convention MSB + 0x20).
    public var lsb: UInt8?
    public var mode: MIDIMode
    /// 7 or 14.
    public var bits: UInt8
    /// Range seen during the gesture, at `bits` resolution.
    public var min: UInt16
    public var max: UInt16
    /// Messages from the control (MSBs only for a 14-bit one).
    public var count: Int
    /// Other CCs that sent exactly the same values at the same time.
    public var mirrors: [MIDIKey]

    /// The crate's serde form, for comparing with its fixtures.
    public var jsonObject: [String: Any] {
        func key(_ k: MIDIKey) -> [String: Any] { ["kind": k.kind.rawValue, "channel": Int(k.channel), "number": Int(k.number)] }
        let mode: [String: Any] = switch self.mode {
        case .auto: ["type": "auto"]
        case .button: ["type": "button"]
        case .absolute: ["type": "absolute"]
        case .relative(let e): ["type": "relative", "encoding": e.rawValue]
        }
        return ["key": key(self.key), "lsb": lsb.map { Int($0) as Any } ?? NSNull(), "mode": mode, "bits": Int(bits),
                "min": Int(min), "max": Int(max), "count": count, "mirrors": mirrors.map(key)]
    }
}

/// Which kind of control the person was asked for, when they were asked for one.
public enum MIDIWant: Sendable { case button, continuous }

public enum MIDIDetect {
    /// Reads one gesture. `noisy` holds controls that send by themselves; they are only chosen
    /// when nothing else moved. `want` breaks ties when two controls moved together.
    public static func classify(_ events: [MIDIEvent], noisy: Set<MIDIKey>, want: MIDIWant?) -> MIDIDetected? {
        var byKey: [MIDIKey: [(UInt64, UInt16)]] = [:]
        for event in events { byKey[event.message.key, default: []].append((event.atMS, event.message.value)) }
        let keys = byKey.keys.sorted()
        // 14-bit pairs: CC n below 0x20 and CC n + 0x20 arriving right after it.
        var lsbOf: [MIDIKey: UInt8] = [:]
        var lsbs = Set<MIDIKey>()
        for key in keys where key.kind == .cc && key.number < 0x20 {
            let msbs = byKey[key]!
            var low = key; low.number = key.number + 0x20
            guard let lowValues = byKey[low] else { continue }
            let paired = lowValues.filter { t, _ in msbs.contains { m, _ in t >= m && t - m <= 2 } }.count
            if paired > 0 && paired * 2 >= lowValues.count { lsbOf[key] = low.number; lsbs.insert(low) }
        }
        var candidates = keys.filter { !lsbs.contains($0) }.map { describe($0, byKey[$0]!, lsb: lsbOf[$0], events: events) }
        // Mirrors: CCs with exactly the same values as a lower-numbered one.
        var mirrored = Set<MIDIKey>()
        for i in candidates.indices where !mirrored.contains(candidates[i].key) {
            let mine = byKey[candidates[i].key]!
            let twins = candidates[(i + 1)...].filter { other in
                other.key.kind == .cc && candidates[i].key.kind == .cc && other.key.channel == candidates[i].key.channel
                    && sameValues(mine, byKey[other.key]!)
            }.map(\.key)
            mirrored.formUnion(twins)
            candidates[i].mirrors = twins
        }
        candidates.removeAll { mirrored.contains($0.key) || tooLittle($0) }
        if candidates.contains(where: { !noisy.contains($0.key) }) { candidates.removeAll { noisy.contains($0.key) } }
        if let want {
            let fits = { (c: MIDIDetected) in (c.mode == .button) == (want == .button) }
            if candidates.contains(where: fits) { candidates.removeAll { !fits($0) } }
        }
        // The control that sent most; on a tie, the one touched last. Like Rust's `max_by`, the
        // last of equal maxima wins.
        func start(_ c: MIDIDetected) -> UInt64 { byKey[c.key]![0].0 }
        var best: MIDIDetected?
        for c in candidates {
            guard let current = best else { best = c; continue }
            if (c.count, start(c)) >= (current.count, start(current)) { best = c }
        }
        return best
    }

    /// Too little to tell what a control is, or to tell it from a worn fader drifting.
    static func tooLittle(_ d: MIDIDetected) -> Bool {
        if d.mode == .button { return d.max == 0 }
        let full: Double = d.bits == 14 ? 16383 : 127
        return d.count < 3 || (d.mode == .absolute && d.count < 10 && Double(d.max - d.min) / full < 0.02)
    }

    static func sameValues(_ a: [(UInt64, UInt16)], _ b: [(UInt64, UInt16)]) -> Bool {
        a.count == b.count && zip(a, b).allSatisfy { x, y in x.1 == y.1 && (x.0 > y.0 ? x.0 - y.0 : y.0 - x.0) <= 5 }
    }

    static func describe(_ key: MIDIKey, _ values: [(UInt64, UInt16)], lsb: UInt8?, events: [MIDIEvent]) -> MIDIDetected {
        let raw = values.map(\.1)
        let mode: MIDIMode, bits: UInt8, seen: [UInt16]
        switch (key.kind, lsb) {
        case (.note, _): (mode, bits, seen) = (.button, 7, raw)
        case (.pitchBend, _): (mode, bits, seen) = (.absolute, 14, raw)
        case (.cc, let lsb?): (mode, bits, seen) = (.absolute, 14, combined(key, lsb: lsb, events: events))
        case (.cc, nil): (mode, bits, seen) = (ccMode(raw), 7, raw)
        }
        return MIDIDetected(key: key, lsb: lsb, mode: mode, bits: bits, min: seen.min() ?? 0, max: seen.max() ?? 0,
                            count: values.count, mirrors: [])
    }

    /// 14-bit values in order: each LSB completes the MSB before it.
    static func combined(_ key: MIDIKey, lsb: UInt8, events: [MIDIEvent]) -> [UInt16] {
        var msb: UInt16?
        var values: [UInt16] = []
        for event in events {
            let k = event.message.key
            guard k.kind == .cc, k.channel == key.channel else { continue }
            if k.number == key.number { msb = event.message.value }
            else if k.number == lsb, let m = msb { values.append(MIDIValue.combineCC14(m, event.message.value)) }
        }
        return values
    }

    static func ccMode(_ values: [UInt16]) -> MIDIMode {
        let distinct = Set(values)
        // A button over CC sends its two states, usually 127 and 0, always a 0 on release.
        if distinct.contains(0) && (distinct.allSatisfy { $0 == 0 || $0 == 127 } || (distinct.count <= 2 && values.count <= 8)) {
            return .button
        }
        return relative(values).map(MIDIMode.relative) ?? .absolute
    }

    /// Absolute controls never repeat a value back to back, nor jump between the ends. Encoders do.
    static func relative(_ values: [UInt16]) -> MIDIEncoding? {
        guard values.count >= 3 else { return nil }
        let repeats = Double(zip(values, values.dropFirst()).filter { $0 == $1 }.count) / Double(values.count - 1)
        let low = { (v: UInt16) in (1...20).contains(v) }
        let high = { (v: UInt16) in (108...127).contains(v) }
        if values.allSatisfy({ low($0) || high($0) }) && ((values.contains(where: low) && values.contains(where: high)) || repeats >= 0.2) {
            return .twosComplement
        }
        if values.allSatisfy({ low($0) || (65...84).contains($0) }) && values.contains(where: low) && values.contains(where: { $0 >= 65 }) {
            return .signBit
        }
        let around = { (v: UInt16) in (44...84).contains(v) }
        let sides = values.contains { $0 < 64 } && values.contains { $0 > 64 }
        if values.allSatisfy(around) && (repeats >= 0.2 || (sides && repeats > 0)) { return .offset64 }
        return nil
    }

    /// Reads a capture as the probe writes it: milliseconds, a tab, then hex bytes.
    public static func parseLog(_ text: String) -> [MIDIEvent] {
        text.split(separator: "\n", omittingEmptySubsequences: true).compactMap { line in
            guard let tab = line.firstIndex(of: "\t") else { return nil }
            let at = line[..<tab].trimmingCharacters(in: .whitespaces)
            var bytes: [UInt8] = []
            for part in line[line.index(after: tab)...].split(whereSeparator: \.isWhitespace) {
                guard let byte = UInt8(part, radix: 16) else { return nil }
                bytes.append(byte)
            }
            guard let ms = UInt64(at), let message = MIDIMessage.parse(bytes) else { return nil }
            return MIDIEvent(atMS: ms, message: message)
        }
    }

    /// Runs a capture through an explorer as if it were live: the first `baselineMS` find noisy
    /// controls, then every gesture is reported with the time it ended.
    public static func replay(_ events: [MIDIEvent], baselineMS: UInt64, quietMS: UInt64) -> [(UInt64, MIDIDetected)] {
        var explorer = MIDIExplorer(quietMS: quietMS)
        explorer.startBaseline(nowMS: 0, lengthMS: baselineMS)
        var found: [(UInt64, MIDIDetected)] = []
        for event in events {
            if let d = explorer.poll(nowMS: event.atMS, want: nil) { found.append((event.atMS, d)) }
            explorer.feed(atMS: event.atMS, event.message)
        }
        if let d = explorer.poll(nowMS: .max, want: nil) { found.append((events.last?.atMS ?? 0, d)) }
        return found
    }
}

/// What the explorer knows about a control it has seen.
public struct MIDISeen: Equatable, Sendable {
    public var key: MIDIKey
    public var lsb: UInt8?
    /// From the last gesture that chose it; nil while it has only been seen.
    public var mode: MIDIMode?
    public var count: UInt64
    public var min: UInt16, max: UInt16, last: UInt16
    /// Sent while nothing was touched.
    public var noisy: Bool
}

/// Watches a controller live: finds noisy controls while nothing is touched, then turns each
/// gesture into a `MIDIDetected` once the controller goes quiet.
public struct MIDIExplorer: Sendable {
    static let maxEvents = 8192, maxSeen = 512
    /// The longest a held note keeps a gesture open with nothing else happening.
    static let heldMS: UInt64 = 5000

    private var events: [MIDIEvent] = []
    private var lastAt: UInt64?
    public private(set) var noisy = Set<MIDIKey>()
    private var baselineUntil: UInt64?
    private var seenByKey: [MIDIKey: MIDISeen] = [:]
    private var lsbs = Set<MIDIKey>()
    private var held = Set<MIDIKey>()
    private var closed = false
    /// Silence that ends a gesture.
    public var quietMS: UInt64

    public init(quietMS: UInt64 = 400) { self.quietMS = quietMS }

    /// Starts listening for controls that move by themselves. Nothing should be touched.
    public mutating func startBaseline(nowMS: UInt64, lengthMS: UInt64) {
        noisy.removeAll()
        reset()
        baselineUntil = nowMS + lengthMS
        for key in seenByKey.keys { seenByKey[key]!.noisy = false }
    }

    public func baselineLeftMS(nowMS: UInt64) -> UInt64? {
        guard let until = baselineUntil, until > nowMS else { return nil }
        return until - nowMS
    }

    public mutating func feed(atMS: UInt64, _ message: MIDIMessage) {
        let key = message.key
        if seenByKey.count < Self.maxSeen || seenByKey[key] != nil {
            var seen = seenByKey[key] ?? MIDISeen(key: key, lsb: nil, mode: nil, count: 0, min: message.value,
                                                  max: message.value, last: message.value, noisy: false)
            seen.count += 1
            seen.min = Swift.min(seen.min, message.value); seen.max = Swift.max(seen.max, message.value)
            seen.last = message.value
            seenByKey[key] = seen
        }
        if baselineLeftMS(nowMS: atMS) != nil {
            noisy.insert(key)
            seenByKey[key]?.noisy = true
            return
        }
        let quiet = noisy.contains(key)
        if quiet && lastAt == nil { return }
        if !quiet { lastAt = atMS }
        if events.count < Self.maxEvents { events.append(MIDIEvent(atMS: atMS, message: message)) }
        if key.kind == .note {
            if message.pressed {
                held.insert(key)
            } else {
                held.remove(key)
                // Only buttons in this gesture, all let go: it is over.
                closed = held.isEmpty && events.allSatisfy { $0.message.key.kind == .note || noisy.contains($0.message.key) }
            }
        } else if !quiet {
            closed = false
        }
    }

    /// A finished gesture: buttons all let go, or the controller quiet for `quietMS`.
    public mutating func poll(nowMS: UInt64, want: MIDIWant?) -> MIDIDetected? {
        guard let last = lastAt else { return nil }
        let quiet = nowMS >= last ? nowMS - last : 0
        let wait = held.isEmpty ? quietMS : Self.heldMS
        if !closed && quiet < wait { return nil }
        let detected = MIDIDetect.classify(events, noisy: noisy, want: want)
        reset()
        if let d = detected {
            if let lsb = d.lsb { var low = d.key; low.number = lsb; lsbs.insert(low) }
            if var seen = seenByKey[d.key] {
                seen.mode = d.mode; seen.lsb = d.lsb
                if d.bits == 14 { seen.min = d.min; seen.max = d.max }
                seenByKey[d.key] = seen
            }
        }
        return detected
    }

    /// Drops a gesture in progress, so the next one starts clean.
    public mutating func reset() { events.removeAll(); lastAt = nil; held.removeAll(); closed = false }

    public var gestureInProgress: Bool { lastAt != nil }

    /// Controls seen so far, without the LSB half of 14-bit pairs.
    public var seen: [MIDISeen] { seenByKey.keys.sorted().filter { !lsbs.contains($0) }.map { seenByKey[$0]! } }

    public mutating func clear() { self = MIDIExplorer(quietMS: quietMS) }
}
