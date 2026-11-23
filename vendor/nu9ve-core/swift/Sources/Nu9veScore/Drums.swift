import Foundation

/// Synthesized percussion. ``params.kind`` picks the model; ``note`` is a
/// tuning offset in semitones around each model's natural pitch, so one
/// instrument can play a whole tom/conga set.
final class DrumVoice: Voice {
    let kind: String
    let tune: Float
    let decay: Float
    let tone: Float
    let drive: Float
    let snappy: Float
    var phase: Float = 0
    var phase2: Float = 0
    var metal: [Osc] = []
    var bp = SVF(), hp = SVF(), lp = SVF(), bp2 = SVF()
    var resonators: [Resonator] = []
    let length: Int
    var clickLP = OnePole()
    var noiseLP = SVF()
    let color: Float
    let humanTone: Float
    let humanDecay: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        kind = p.s("kind", "kick")
        tune = powf(2, (event.note - 60) / 12) * p.f("tune", 1.0)
        decay = p.f("decay", 1.0)
        tone = p.f("tone", 0.5)
        drive = p.f("drive", 0.0)
        snappy = p.f("snappy", 0.6)
        color = p.f("color", 9000)
        // Round-robin feel: every hit is a little different (seeded per event).
        var variation = Rng(seed: hash64(seed, UInt64(event.index), 0xD5))
        humanTone = 1 + variation.bipolar() * 0.05
        humanDecay = 1 + variation.bipolar() * 0.12
        let seconds: Float
        switch kind {
        case "kick": seconds = 0.55 * decay + 0.1
        case "rattle": seconds = 0.08 * decay + 0.02
        case "snap": seconds = 0.12
        case "gsnare": seconds = 0.42
        case "snare", "clap": seconds = 0.45 * decay + 0.1
        case "brush": seconds = 0.4 * decay + 0.05
        case "rimshot": seconds = 0.25 * decay + 0.05
        case "hat": seconds = 0.09 * decay + 0.02
        case "ohat": seconds = 0.6 * decay + 0.05
        case "ride": seconds = 2.2 * decay
        case "crash": seconds = 3.2 * decay
        case "tom", "conga", "bongo", "tambora": seconds = 0.7 * decay
        case "timbale": seconds = 0.8 * decay
        case "cowbell": seconds = 0.5 * decay
        case "metal": seconds = 1.8 * decay
        default: seconds = 0.35 * decay + 0.05
        }
        length = Int(seconds * sampleRate)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        setup(spec: spec)
    }

    private func setup(spec: InstrumentSpec) {
        let sr = sampleRate
        switch kind {
        case "hat", "ohat", "ride", "crash":
            // TR-808 metal: six detuned squares through band/high-pass.
            let base: [Float] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0]
            metal = base.map { _ in Osc(phase: rng.unit()) }
            bp.set(cutoff: (kind == "ride" ? 5200 : 9500) * tune, q: 1.2, sampleRate: sr)
            hp.set(cutoff: (kind == "crash" ? 3800 : kind == "ride" ? 3000 : 7200) * tune, q: 0.7, sampleRate: sr)
            bp2.set(cutoff: 3100 * tune, q: 4, sampleRate: sr)
        case "rattle":
            hp.set(cutoff: 5200 * tune, q: 0.8, sampleRate: sr)
        case "snap":
            bp.set(cutoff: 2300 * tune, q: 2.2, sampleRate: sr)
        case "snare", "rimshot", "gsnare":
            bp.set(cutoff: (1800 + 2600 * tone) * humanTone, q: 0.8, sampleRate: sr)
            hp.set(cutoff: 900, q: 0.7, sampleRate: sr)
            bp2.set(cutoff: 1700 * tune, q: 5, sampleRate: sr)
        case "brush":
            bp.set(cutoff: 3200 * tune, q: 0.6, sampleRate: sr)
        case "clap":
            bp.set(cutoff: 1150 * tune, q: 2.2, sampleRate: sr)
        case "cowbell":
            bp.set(cutoff: 2600 * tune, q: 2.5, sampleRate: sr)
        case "shaker", "guiro":
            bp.set(cutoff: (kind == "shaker" ? 6500 : 3200) * tune, q: 1.4, sampleRate: sr)
        case "rim":
            bp.set(cutoff: 1700 * tune, q: 6, sampleRate: sr)
        case "conga", "bongo", "tambora", "tom", "timbale":
            let f: Float = (kind == "bongo" ? 360 : kind == "tambora" ? 110 : kind == "tom" ? 130 : kind == "timbale" ? 420 : 200) * tune
            let ratios: [Float] = kind == "timbale" ? [1, 1.72, 2.61, 3.3] : [1, 1.5, 1.99, 2.44]
            let amps: [Float] = kind == "timbale" ? [1, 0.55, 0.4, 0.25] : [1, 0.35, 0.18, 0.08]
            let decayBase: Float = (kind == "tom" ? 0.38 : kind == "tambora" ? 0.3 : kind == "timbale" ? 0.6 : 0.32) * decay
            for (k, ratio) in ratios.enumerated() {
                resonators.append(Resonator(frequency: f * ratio, decay: decayBase / Float(k + 1) * (k == 0 ? 1 : 0.6),
                                            amplitude: amps[k], phase: 0, sampleRate: sr))
            }
            bp.set(cutoff: 2200 * tune, q: 1.5, sampleRate: sr)
        case "metal":
            let f: Float = 160 * tune
            let ratios: [Float] = [1, 1.59, 2.14, 2.65, 3.17, 3.9, 5.3, 6.8]
            for (k, ratio) in ratios.enumerated() {
                resonators.append(Resonator(frequency: f * ratio * (1 + rng.bipolar() * 0.01), decay: 1.4 * decay / (1 + Float(k) * 0.25),
                                            amplitude: 1 / Float(k + 1).squareRoot(), phase: rng.unit() * twoPi, sampleRate: sr))
            }
            bp.set(cutoff: 2400, q: 0.9, sampleRate: sr)
        case "glitch":
            bp.set(cutoff: 600 + 6000 * rng.unit(), q: 3, sampleRate: sr)
        default:
            break
        }
        clickLP.setLowpass(cutoff: 7000, sampleRate: sr)
        noiseLP.set(cutoff: color, q: 0.6, sampleRate: sr)
        lp.set(cutoff: kind == "crash" ? 9500 : 8200 + 2000 * tone, q: 0.6, sampleRate: sr)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let sr = sampleRate
        for i in 0..<count {
            let t = Float(elapsed) / sr
            var s: Float = 0
            switch kind {
            case "kick" where revision == .v1:
                let f0: Float = 46 * tune, f1: Float = 46 * tune * (2.4 + 3 * tone)
                let f = f0 + (f1 - f0) * expf(-t / 0.032)
                phase += f / sr; if phase >= 1 { phase -= 1 }
                let body = sinf(twoPi * phase) * expf(-t / (0.32 * decay))
                let click = clickLP.lowpass(rng.bipolar()) * expf(-t / 0.0025) * 0.6
                s = saturate(body * 1.25 + click, 0.25 + drive) * 0.95
            case "kick":
                // Two-stage pitch drop (beater then body) into a key-tuned note,
                // a short click for small speakers and an optional long sub tail.
                let f0: Float = 46 * tune, f1: Float = 46 * tune * (2.4 + 3 * tone)
                let f = f0 + (f1 - f0) * (0.7 * expf(-t / 0.026) + 0.3 * expf(-t / 0.006))
                phase += f / sr; if phase >= 1 { phase -= 1 }
                let tail = snappy > 0.95 ? 0 : max(0, 0.6 - snappy) * expf(-t / (0.9 * decay))
                let body = sinf(twoPi * phase) * (expf(-t / (0.32 * decay)) * (1 - tail) + tail)
                let click = clickLP.lowpass(rng.bipolar()) * expf(-t / 0.0025) * 0.6 * (0.5 + color / 18000)
                s = saturate(body * 1.25 + click, 0.25 + drive) * 0.95
            case "rattle":
                // Ayoyotes: four seed grains in quick succession.
                var env: Float = 0
                for (k, offset) in [Float(0), 0.007, 0.016, 0.027].enumerated() {
                    let tk = t - offset
                    if tk >= 0 { env += expf(-tk / (0.012 * decay)) / sqrtf(1 + Float(k) * 0.3) }
                }
                s = hp.highpass(rng.bipolar()) * env * 0.5
            case "snap":
                let n = bp.bandpass(rng.bipolar())
                s = (n * 2.2 + (elapsed < 24 ? rng.bipolar() * 0.6 : 0)) * expf(-t / 0.011)
            case "gsnare":
                // 80s gated snare: the head and wires, then a dense room cut dead.
                let f = 190 * tune * humanTone * (1 + 0.3 * expf(-t / 0.012))
                phase += f / sr; if phase >= 1 { phase -= 1 }
                let body = sinf(twoPi * phase) * expf(-t / 0.06)
                let n = rng.bipolar()
                let gate: Float = t < 0.24 ? (1 - t * 1.2) : max(0, 1 - (t - 0.24) / 0.018) * 0.71
                let room = bp.bandpass(n) * 0.9 + hp.highpass(n) * 0.5
                s = saturate(body * 0.9 + room * (0.5 * expf(-t / 0.02) + 0.55 * gate), 0.2) * 0.8
            case "snare", "rimshot":
                // Head (two modes with a short pitch drop) + wires. The wires get
                // darker as they ring and fall off fast: a crack, not sand.
                let f = 185 * tune * humanTone * (1 + 0.18 * expf(-t / 0.018))
                phase += f / sr; if phase >= 1 { phase -= 1 }
                phase2 += f * 1.59 / sr; if phase2 >= 1 { phase2 -= 1 }
                let d = decay * humanDecay
                let body = (sinf(twoPi * phase) + 0.45 * sinf(twoPi * phase2)) * expf(-t / (0.085 * d))
                if elapsed % 16 == 0 {
                    noiseLP.set(cutoff: max(1800, color * expf(-t / (0.09 * d)) + 1500), q: 0.6, sampleRate: sr)
                }
                let n = noiseLP.lowpass(rng.bipolar())
                let wires = (bp.bandpass(n) * 0.75 + hp.highpass(n) * 0.45)
                    * (0.55 * expf(-t / 0.012) + 0.45 * expf(-t / (0.075 * d)))
                if kind == "rimshot" {
                    let click = bp2.bandpass(rng.bipolar()) * expf(-t / 0.006) * 2.2
                    s = saturate(body * 0.8 * expf(-t / 0.05) + wires * 0.35 + click, drive + 0.1) * 0.8
                } else {
                    s = saturate(body * (1 - snappy) * 1.1 + wires * snappy * 1.45, drive) * 0.75
                }
            case "brush":
                // A swish: soft attack, filtered noise, a hint of head.
                let env = min(1, t / 0.014) * expf(-t / (0.11 * decay * humanDecay))
                phase += 190 * tune / sr; if phase >= 1 { phase -= 1 }
                s = (bp.bandpass(rng.bipolar()) * 0.9 + sinf(twoPi * phase) * 0.18 * expf(-t / 0.05)) * env
            case "clap":
                let n = bp.bandpass(rng.bipolar())
                var env: Float = 0
                for k in 0..<3 { let tk = t - Float(k) * 0.011; if tk >= 0 { env += expf(-tk / 0.0045) } }
                env += expf(-max(0, t - 0.022) / (0.13 * decay)) * (t > 0.022 ? 0.7 : 0)
                s = n * env * 1.6
            case "hat", "ohat", "ride", "crash":
                var m: Float = 0
                let base: [Float] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0]
                for k in 0..<metal.count { m += metal[k].pulse(base[k] * tune * 2 / sr, width: 0.5) }
                let noise = rng.bipolar() * (kind == "crash" ? 0.9 : 0.4)
                let raw = (m * 0.18 + noise)
                var y = hp.highpass(bp.bandpass(raw) * 1.5 + raw * 0.3)
                if kind == "ride" { y += bp2.bandpass(m * 0.15) * expf(-t / 0.4) * 1.5 }
                let d: Float = kind == "hat" ? 0.03 : kind == "ohat" ? 0.22 : kind == "ride" ? 0.7 : 1.1
                // Tame the fizz: cymbals keep their air but lose the hard top.
                s = lp.lowpass(y) * expf(-t / (d * decay)) * (kind == "crash" ? 0.55 : 0.6)
            case "conga", "bongo", "tambora", "tom", "timbale":
                for k in 0..<resonators.count { s += resonators[k].next() }
                let slap = tone > 0.6 ? (tone - 0.6) * 2.5 : 0
                s += bp.bandpass(rng.bipolar()) * expf(-t / 0.012) * (0.25 + slap)
                if kind == "tom" || kind == "tambora" {
                    // membranes drop in pitch as the skin relaxes: emulate with a body sine
                    let f = (kind == "tom" ? 130 : 110) * tune * (1 + 0.5 * expf(-t / 0.05))
                    phase += f / sr; if phase >= 1 { phase -= 1 }
                    s += sinf(twoPi * phase) * expf(-t / (0.3 * decay)) * 0.6
                }
                s *= kind == "timbale" ? 0.45 : 0.6
            case "cowbell":
                phase += 587 * tune / sr; if phase >= 1 { phase -= 1 }
                phase2 += 845 * tune / sr; if phase2 >= 1 { phase2 -= 1 }
                let sq = (phase < 0.5 ? 1 : -1) + (phase2 < 0.5 ? 1 : -1) as Float
                let env = t < 0.012 ? 1 : expf(-(t - 0.012) / (0.11 * decay))
                s = bp.bandpass(sq) * env * 0.45
            case "clave":
                phase += 2480 * tune / sr; if phase >= 1 { phase -= 1 }
                s = sinf(twoPi * phase) * expf(-t / 0.028) * 0.55
            case "shaker":
                let env = min(1, t / 0.012) * expf(-t / (0.055 * decay))
                s = bp.bandpass(rng.bipolar()) * env * 0.7
            case "guiro":
                let tick = (sinf(twoPi * 38 * t) > 0.6 ? 1 : 0.2) as Float
                s = bp.bandpass(rng.bipolar()) * tick * (t < 0.14 * decay ? 1 : 0) * 0.5
            case "rim":
                phase += 520 * tune / sr; if phase >= 1 { phase -= 1 }
                s = (bp.bandpass(rng.bipolar()) * 1.4 + sinf(twoPi * phase) * 0.4) * expf(-t / 0.018)
            case "metal":
                for k in 0..<resonators.count { s += resonators[k].next() }
                s = saturate(s * 0.4 + bp.bandpass(rng.bipolar()) * expf(-t / 0.02), 0.3 + drive) * 0.5
            case "glitch":
                let hold = Int(sr / (800 + 6000 * tone))
                if hold > 0 && elapsed % hold == 0 { phase = rng.bipolar() }
                s = bp.bandpass(phase) * expf(-t / (0.05 * decay)) * 1.2
            case "tick":
                phase += 5200 * tune / sr; if phase >= 1 { phase -= 1 }
                s = (sinf(twoPi * phase) * 0.5 + (elapsed < 2 ? 0.8 : 0)) * expf(-t / 0.006)
            default:
                s = rng.bipolar() * expf(-t / 0.05)
            }
            let a = velocity * velocity * 0.9 + 0.1 * velocity
            l[i] += s * a * panL * 1.414
            r[i] += s * a * panR * 1.414
            elapsed += 1
        }
        return elapsed < length
    }
}
