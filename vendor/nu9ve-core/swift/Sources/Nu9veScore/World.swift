import Foundation

// Acoustic-world voices ported from the Sound Lab recipes (membranes, slit
// drums, rattles, calls) plus a taiko for cinematic scores. Pitch comes from
// the event note; articulation from the instrument params.

private let membraneRatios: [Float] = [1, 1.59, 2.14, 2.3, 2.65]

/// Huehuetl / atabaque / taiko: a pitch-dropping head plus membrane modes and
/// a strike. ``params.mode``: huehuetl | atabaque | taiko; ``open``: 0 rim/mute.
final class MembraneVoice: Voice {
    let mode: String
    let open: Bool
    var phase: Float = 0
    var modes: [Resonator] = []
    var strike = SVF(), slap = SVF()
    let f: Float
    let thumpT60: Float
    let bend: Float
    let length: Int
    let level: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        mode = p.s("mode", "huehuetl")
        open = p.f("open", 1) > 0.5
        f = midiToHz(event.note)
        let taiko = mode == "taiko"
        let decay = p.f("decay", 1)
        thumpT60 = (taiko ? 1.5 : open ? 0.5 : 0.16) * decay
        bend = taiko ? 1.7 : 1.45
        level = p.f("level", 1)
        let modeDecays: [Float] = taiko ? [1.3, 0.8, 0.5, 0.35, 0.25] : open ? [0.5, 0.3, 0.2, 0.15, 0.1] : [0.16, 0.1, 0.07, 0.05, 0.04]
        let gains: [Float] = taiko ? [1, 0.6, 0.4, 0.3, 0.2] : [1, 0.5, 0.35, 0.25, 0.15]
        length = Int((thumpT60 * 1.1 + 0.1) * sampleRate)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        for (k, ratio) in membraneRatios.enumerated() {
            modes.append(Resonator(frequency: f * 1.02 * ratio * (1 + rng.bipolar() * 0.004), decay: modeDecays[k] * decay,
                                   amplitude: gains[k], phase: rng.unit() * twoPi, sampleRate: sampleRate))
        }
        strike.set(cutoff: taiko ? 1300 : open ? 900 : 2600, q: 0.7, sampleRate: sampleRate)
        slap.set(cutoff: mode == "atabaque" ? 1800 : 2600, q: 1.2, sampleRate: sampleRate)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let sr = sampleRate
        let taiko = mode == "taiko"
        for i in 0..<count {
            let t = Float(elapsed) / sr
            // Head: an exponential pitch drop from the strike into the note.
            let pitch = f * (1 + (bend - 1) * expf(-t / max(0.006, thumpT60 * 0.12)))
            phase += pitch / sr; if phase >= 1 { phase -= 1 }
            let attack = min(1, t / 0.0015)
            var s = sinf(twoPi * phase) * attack * expf(-t * 6.91 / thumpT60) * 0.36
            var m: Float = 0
            for k in 0..<modes.count { m += modes[k].next() }
            s += m * 0.22
            let strikeLevel: Float = taiko ? 0.6 : open ? 0.3 : 0.5
            let strikeT60: Float = taiko ? 0.05 : open ? 0.03 : 0.015
            s += strike.lowpass(rng.bipolar()) * strikeLevel * expf(-t * 6.91 / strikeT60)
            if !open || mode == "atabaque" && note > 60 {
                s += slap.bandpass(rng.bipolar()) * 0.25 * expf(-t / 0.006)
            }
            // Velocity shapes timbre too: soft hits are darker and rounder.
            let a = (velocity * velocity * 0.85 + 0.15 * velocity) * level * (taiko ? 1.6 : 1.3)
            l[i] += s * a * panL * 1.414
            r[i] += s * a * panR * 1.414
            elapsed += 1
        }
        return elapsed < length
    }
}

/// Bird calls (chirp, trill, whistle, water whistle). ``params.call``.
final class BirdVoice: Voice {
    let call: String
    var phase: Float = 0
    var breath = SVF()
    let f: Float
    let repeats: Int
    let spacing: Float
    let warble: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        call = spec.params.s("call", "chirp")
        f = midiToHz(event.note)
        var r = Rng(seed: hash64(seed, UInt64(event.index), 0xB1))
        repeats = 2 + Int(r.unit() * 3)
        spacing = 0.07 + r.unit() * 0.03
        warble = (call == "trill" ? 24 : 8) + r.unit() * (call == "trill" ? 8 : 4)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        breath.set(cutoff: f * 2, q: 3, sampleRate: sampleRate)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let sr = sampleRate
        let total: Float = call == "chirp" ? Float(repeats) * spacing + 0.08 : call == "whistle" ? 0.5 : call == "trill" ? 0.4 : 0.66
        for i in 0..<count {
            let t = Float(elapsed) / sr
            var s: Float = 0
            switch call {
            case "chirp":
                let k = Int(t / spacing)
                let tk = t - Float(k) * spacing
                if k < repeats && tk < 0.07 {
                    let hz = f * 0.8 * (1 + 0.7 * expf(-tk / 0.012))
                    phase += hz / sr; if phase >= 1 { phase -= 1 }
                    s = sinf(twoPi * phase) * min(1, tk / 0.004) * expf(-max(0, tk - 0.024) / 0.015) * 0.06
                }
            case "trill":
                let hz = f * powf(2, 2.8 / 12 * sinf(twoPi * warble * t))
                phase += hz / sr; if phase >= 1 { phase -= 1 }
                s = sinf(twoPi * phase) * min(1, t / 0.03) * (t < 0.31 ? 1 : max(0, 1 - (t - 0.31) / 0.06)) * 0.05
            case "whistle":
                let first = t < 0.2
                let tk = first ? t : t - 0.24
                if tk >= 0 && tk < 0.2 {
                    let target = f * (first ? 1.15 : 0.9)
                    let from: Float = first ? 0.66 : 1.22
                    let hz = target * (1 + (from - 1) * expf(-tk / 0.045))
                    phase += hz / sr; if phase >= 1 { phase -= 1 }
                    s = sinf(twoPi * phase) * min(1, tk / 0.018) * expf(-max(0, tk - 0.12) / 0.03) * 0.06
                }
            default: // water whistle: breathy, gurgling warble
                let hz = f * powf(2, 0.7 / 12 * sinf(twoPi * warble * t + sinf(twoPi * 3.1 * t)))
                phase += hz / sr; if phase >= 1 { phase -= 1 }
                let env = min(1, t / 0.05) * (t < 0.5 ? 1 : max(0, 1 - (t - 0.5) / 0.15))
                s = (sinf(twoPi * phase) + 0.12 * sinf(4 * .pi * phase)) * env * 0.06 + breath.bandpass(rng.bipolar()) * env * 0.04
            }
            let a = velocity * 1.4
            l[i] += s * a * panL * 1.414
            r[i] += s * a * panR * 1.414
            elapsed += 1
        }
        return Float(elapsed) / sr < total
    }
}

/// Atecocolli: the conch trumpet — a reed call that rises and swells.
final class ConchVoice: Voice {
    var osc = Osc(), osc2 = Osc()
    var band = SVF(), air = SVF()
    let f: Float
    let length: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        f = midiToHz(event.note)
        length = Float(event.duration)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        air.set(cutoff: f * 3, q: 2, sampleRate: sampleRate)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let sr = sampleRate
        var i = 0
        while i < count {
            let block = min(16, count - i)
            let t = Float(elapsed) / sr
            let u = clamp01(t / max(0.1, length))
            band.set(cutoff: 780 + 470 * u, q: 1.3, sampleRate: sr)
            let vib = smoothstep((t - length * 0.4) / 0.3) * 0.12 * sinf(twoPi * 4.8 * t)
            let hz = f * (1 - 0.08 * expf(-t / (length * 0.12))) * powf(2, vib / 12)
            for j in 0..<block {
                let tt = Float(elapsed) / sr
                let env = smoothstep(tt / min(0.3, length * 0.3)) * (tt < length * 0.65 ? 1 : max(0, 1 - (tt - length * 0.65) / (length * 0.35)))
                let reed = 0.6 * osc.saw(hz / sr) + 0.4 * osc2.pulse(hz / sr, width: 0.32)
                let s = band.bandpass(reed) * 0.5 + air.bandpass(rng.bipolar()) * 0.06 * env
                let a = env * velocity * 0.9
                l[i + j] += s * a * panL * 1.414
                r[i + j] += s * a * panR * 1.414
                elapsed += 1
            }
            i += block
        }
        return Float(elapsed) / sr < length + 0.05
    }
}

/// Pea whistle (apito): bright fluttering sine, gated.
final class WhistleVoice: Voice {
    var phase: Float = 0
    var band = SVF()
    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        band.set(cutoff: 2950, q: 6, sampleRate: sampleRate)
    }
    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let sr = sampleRate
        let total = gateSamples + Int(0.03 * sr)
        for i in 0..<count {
            let t = Float(elapsed) / sr
            let hz = 2950 * powf(2, 0.9 / 12 * sinf(twoPi * 36 * t))
            phase += hz / sr; if phase >= 1 { phase -= 1 }
            let env = min(1, t / 0.004) * (elapsed < gateSamples ? 1 : max(0, 1 - Float(elapsed - gateSamples) / (0.03 * sr)))
            let s = (sinf(twoPi * phase) * 0.07 + band.bandpass(rng.bipolar()) * 0.02) * env * velocity * 1.5
            l[i] += s * panL * 1.414
            r[i] += s * panR * 1.414
            elapsed += 1
        }
        return elapsed < total
    }
}
