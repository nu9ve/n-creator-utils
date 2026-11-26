import Foundation

/// A sounding note. Renders by *adding* into the instrument buffers and
/// returns false once it is silent for good.
class Voice {
    let sampleRate: Float
    let velocity: Float
    let note: Float
    var glide: Float          // semitones away from ``note`` at the start
    let glideSamples: Float
    let gateSamples: Int
    var elapsed = 0
    var rng: Rng
    let panL: Float, panR: Float
    let revision: ScoreRevision

    init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        self.sampleRate = sampleRate
        revision = spec.revision
        velocity = event.velocity
        note = event.note
        glide = event.glide
        glideSamples = max(1, spec.params.f("glide_time", 0.06) * sampleRate)
        gateSamples = max(1, Int(event.duration * Double(sampleRate)))
        rng = Rng(seed: hash64(seed, UInt64(event.index), 0x5C0E))
        let pan = max(-1, min(1, event.pan + spec.pan))
        panL = cosf((pan + 1) * .pi / 4)
        panR = sinf((pan + 1) * .pi / 4)
    }

    var gateOpen: Bool { elapsed < gateSamples }

    /// Current pitch in semitones, sliding from ``note + glide`` to ``note``.
    @inline(__always) func pitch() -> Float {
        if glide == 0 { return note }
        let u = min(1, Float(elapsed) / glideSamples)
        return note + glide * (1 - u * u * (3 - 2 * u))
    }

    func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        false
    }
}

/// Block-rate values shared by all voices of an instrument (automation).
struct InstrumentControl {
    var cutoff: Float = 1      // multiplier
    var time: Float = 0        // absolute seconds of the block start
    var bpm: Float = 120
    var morph: Float = 0       // added to a wavetable position (``morph`` lane)
}

// MARK: - pads and stabs

private let formantTable: [String: [(Float, Float)]] = [
    "a": [(800, 1.0), (1150, 0.5), (2900, 0.25)],
    "e": [(400, 1.0), (1600, 0.4), (2700, 0.25)],
    "i": [(270, 1.0), (2140, 0.3), (2950, 0.2)],
    "o": [(450, 1.0), (800, 0.6), (2830, 0.15)],
    "u": [(325, 1.0), (700, 0.5), (2530, 0.1)],
]

final class PadVoice: Voice {
    var oscs: [Osc]
    let detunes: [Float]
    let pans: [(Float, Float)]
    let wave: String
    var filterL = Ladder(), filterR = Ladder()
    var amp: ADSR
    var fenv: ADSR
    let cutoff: Float, resonance: Float, envAmount: Float, keyTrack: Float
    let drive: Float, noiseAmount: Float, subAmount: Float, width: Float
    var subOsc = Osc()
    var formantsL: [SVF] = [], formantsR: [SVF] = []
    var formantGains: [Float] = []
    let formantMix: Float
    let lfoRate: Float, lfoDepth: Float, lfoPhase: Float, vibrato: Float
    /// Modulation: LFO1 shape (filter); LFO2 drifts pitch (cents), pan and pulse width.
    let lfoShape: String, lfo2Rate: Float, lfo2Pitch: Float, lfo2Pan: Float, pwm: Float, lfoSeed: UInt64
    var hp = OnePole()
    /// Layers (all off unless the manifest asks): a second oscillator `layer_semi`
    /// away mixed under the unison stack before the filter, analog drift per
    /// unison voice, and the wavetable position of `wave: "wt"` (0 sine … 3 square)
    /// moved by the filter envelope, LFO1 and the instrument's ``morph`` lane.
    let layerWave: String, layerSemi: Float, layerMix: Float
    var layerOsc = Osc()
    let drift: Float
    let driftPhases: [Float]
    let morph: Float, morphEnv: Float, morphLfo: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        let unison = max(1, min(7, Int(p.f("unison", 5))))
        let cents = p.f("detune", 12)
        wave = p.s("wave", "saw")
        var r = Rng(seed: hash64(seed, UInt64(event.index), 77))
        oscs = (0..<unison).map { _ in Osc(phase: r.unit()) }
        detunes = (0..<unison).map { i in
            unison == 1 ? 0 : (Float(i) / Float(unison - 1) * 2 - 1) * cents / 100
        }
        width = p.f("width", 0.8)
        pans = (0..<unison).map { i in
            let pos = unison == 1 ? 0 : (Float(i) / Float(unison - 1) * 2 - 1) * 0.8
            return (cosf((pos + 1) * .pi / 4), sinf((pos + 1) * .pi / 4))
        }
        amp = ADSR(attack: p.f("attack", 0.8), decay: p.f("decay", 1.5), sustain: p.f("sustain", 0.8),
                   release: p.f("release", 2.5), sampleRate: sampleRate)
        fenv = ADSR(attack: p.f("f_attack", p.f("attack", 0.8)), decay: p.f("f_decay", 2.0), sustain: p.f("f_sustain", 0.4),
                    release: p.f("release", 2.5), sampleRate: sampleRate)
        cutoff = p.f("cutoff", 1400)
        resonance = p.f("resonance", 0.15)
        envAmount = p.f("env_amount", 1.2)
        keyTrack = p.f("key_track", 0.5)
        drive = p.f("drive", 0.15)
        noiseAmount = p.f("noise", 0.0)
        subAmount = p.f("sub", 0.0)
        lfoRate = p.f("lfo_rate", 0.11)
        lfoDepth = p.f("lfo_depth", 0.35)
        vibrato = p.f("vibrato", 0.0)
        lfoPhase = r.unit()
        lfoShape = p.s("lfo_shape", "sine")
        lfo2Rate = p.f("lfo2_rate", 0); lfo2Pitch = p.f("lfo2_pitch", 0); lfo2Pan = p.f("lfo2_pan", 0); pwm = p.f("pwm", 0)
        lfoSeed = hash64(seed, UInt64(event.index), 91)
        layerWave = p.s("layer_wave", ""); layerSemi = p.f("layer_semi", 12); layerMix = layerWave.isEmpty ? 0 : p.f("layer_mix", 0.3)
        drift = p.f("drift", 0)
        var d = Rng(seed: hash64(seed, UInt64(event.index), 0xD71F))
        driftPhases = drift > 0 ? (0..<unison).map { _ in d.unit() } : []
        morph = p.f("morph", 1.5); morphEnv = p.f("morph_env", 0); morphLfo = p.f("morph_lfo", 0)
        let vowel = p.s("vowel", "")
        formantMix = vowel.isEmpty ? 0 : p.f("vowel_mix", 0.85)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        hp.setLowpass(cutoff: 60, sampleRate: sampleRate)
        if let table = formantTable[vowel] {
            // Formants move a little with the voice so choirs are not static.
            for (frequency, gain) in table {
                var a = SVF(), b = SVF()
                a.set(cutoff: frequency, q: 6, sampleRate: sampleRate)
                b.set(cutoff: frequency * 1.02, q: 6, sampleRate: sampleRate)
                formantsL.append(a)
                formantsR.append(b)
                formantGains.append(gain)
            }
        }
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        var dts = [Float](repeating: 0, count: oscs.count)
        var subDt: Float = 0
        let gain = 0.22 / sqrtf(Float(oscs.count)) * (0.55 + 0.45 * velocity)
        var i = 0
        while i < count {
            let block = min(16, count - i)
            let t = control.time + Float(i) / sampleRate
            let lfo = lfoValue(lfoRate * t + lfoPhase, lfoShape, seed: lfoSeed)
            let lfo2 = lfo2Rate > 0 ? sinf(twoPi * (lfo2Rate * t + lfoPhase * 0.37)) : 0
            let semis = pitch() + vibrato * sinf(twoPi * 5.2 * t) + lfo2 * lfo2Pitch / 100
            let sway = lfo2 * lfo2Pan
            let base = midiToHz(semis)
            for k in 0..<oscs.count {
                let wander = drift > 0 ? analogDrift(t, k, cents: drift, phase: driftPhases[k]) : 0
                dts[k] = base * powf(2, (detunes[k] + wander) / 12) / sampleRate
            }
            subDt = base * 0.5 / sampleRate
            let layerDt = base * powf(2, layerSemi / 12) / sampleRate
            let position = wave == "wt" ? morph + morphEnv * fenv.value + morphLfo * lfo + control.morph : 0
            let envValue = fenv.value
            let track = powf(base / 261.6, keyTrack)
            let fc = cutoff * control.cutoff * track * powf(2, envAmount * envValue + lfoDepth * lfo) * (0.7 + 0.3 * velocity)
            filterL.set(cutoff: fc, resonance: resonance, sampleRate: sampleRate)
            filterR.set(cutoff: fc * 1.01, resonance: resonance, sampleRate: sampleRate)
            for j in 0..<block {
                if elapsed == gateSamples { amp.gateOff(); fenv.gateOff() }
                var sl: Float = 0, sr: Float = 0
                for k in 0..<oscs.count {
                    let s: Float
                    switch wave {
                    case "tri": s = oscs[k].triangle(dts[k])
                    case "square": s = oscs[k].pulse(dts[k], width: 0.5 + pwm * lfo2)
                    case "mix": s = 0.6 * oscs[k].saw(dts[k]) + 0.4 * sinf(twoPi * oscs[k].phase)
                    case "wt": s = oscs[k].morph(dts[k], position: position)
                    default: s = oscs[k].saw(dts[k])
                    }
                    sl += s * pans[k].0
                    sr += s * pans[k].1
                }
                if layerMix > 0 {
                    // Centred: the stack keeps its width, the layer its weight.
                    let s = layerOsc.wave(layerWave, layerDt) * layerMix * sqrtf(Float(oscs.count))
                    sl += s * 0.707; sr += s * 0.707
                }
                let mono = (sl + sr) * 0.5
                sl = mono + (sl - mono) * width
                sr = mono + (sr - mono) * width
                if noiseAmount > 0 { let n = rng.bipolar() * noiseAmount; sl += n; sr += n * 0.8 + rng.bipolar() * noiseAmount * 0.2 }
                var yl = filterL.process(sl, drive: drive)
                var yr = filterR.process(sr, drive: drive)
                if subAmount > 0 { let s = subOsc.sine(subDt) * subAmount; yl += s; yr += s }
                if formantMix > 0 {
                    var fl: Float = 0, fr: Float = 0
                    for f in 0..<formantsL.count {
                        fl += formantsL[f].bandpass(yl) * formantGains[f]
                        fr += formantsR[f].bandpass(yr) * formantGains[f]
                    }
                    yl = yl * (1 - formantMix) + fl * formantMix * 2.2
                    yr = yr * (1 - formantMix) + fr * formantMix * 2.2
                }
                let a = amp.next()
                _ = fenv.next()
                let index = i + j
                l[index] += yl * a * gain * panL * 1.414 * (1 - sway)
                r[index] += yr * a * gain * panR * 1.414 * (1 + sway)
                elapsed += 1
            }
            i += block
        }
        return !amp.finished
    }
}

// MARK: - FM electric piano

final class KeysVoice: Voice {
    var carrier: Float = 0, modulator: Float = 0, tine: Float = 0
    let ratio: Float, index: Float, tineAmount: Float, decay: Float, release: Float
    let tremRate: Float, tremDepth: Float, drive: Float, bell: Float
    var level: Float = 0
    var releasing = false
    var releaseCoef: Float = 1
    var decayCoef: Float = 1
    var lp = OnePole()

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        ratio = p.f("ratio", 1.0)
        index = p.f("index", 1.8)
        tineAmount = p.f("tine", 0.35)
        decay = p.f("decay", 3.2) * (1 - 0.35 * clamp01((event.note - 60) / 30))
        release = p.f("release", 0.35)
        tremRate = p.f("tremolo_rate", 4.6)
        tremDepth = p.f("tremolo", 0.12)
        drive = p.f("drive", 0.25)
        bell = p.f("bell", 0.0)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        decayCoef = expf(-1 / (decay * sampleRate / 6.9))
        releaseCoef = expf(-1 / max(1, release * sampleRate / 6.9))
        lp.setLowpass(cutoff: 3500 + 6000 * velocity, sampleRate: sampleRate)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let f = midiToHz(pitch())
        let dt = f / sampleRate
        for i in 0..<count {
            let t = Float(elapsed) / sampleRate
            if elapsed == gateSamples { releasing = true }
            let attack = min(1, t / 0.0025)
            if elapsed == 0 { level = 1 }
            level *= releasing ? releaseCoef : decayCoef
            let modIndex = index * (0.25 + 0.75 * velocity) * (0.35 + 0.65 * expf(-t / 0.35))
            let tineIndex = tineAmount * velocity * expf(-t / 0.025)
            let tineSignal = sinf(twoPi * tine)
            let mod = sinf(twoPi * modulator) * modIndex + tineSignal * tineIndex
            var s = sinf(twoPi * carrier + mod)
            if bell > 0 { s += bell * sinf(twoPi * carrier * 3.5) * expf(-t / 0.6) }
            s = warm(s * (0.6 + 0.6 * velocity), drive)
            s = lp.lowpass(s)
            let trem = 1 + tremDepth * sinf(twoPi * (tremRate * (control.time + Float(i) / sampleRate)))
            let a = attack * level * 0.32 * velocity
            l[i] += s * a * panL * 1.414 * trem
            r[i] += s * a * panR * 1.414 * (2 - trem)
            carrier += dt; if carrier >= 1 { carrier -= 1 }
            modulator += dt * ratio; if modulator >= 1 { modulator -= floorf(modulator) }
            tine += dt * 14; if tine >= 1 { tine -= floorf(tine) }
            elapsed += 1
        }
        return level > 1e-5 || elapsed < 64
    }
}

// MARK: - modal (glass, mallets, bells, metal)

private let materials: [String: (ratios: [Float], amps: [Float], decays: [Float])] = [
    "glass": ([1, 2.756, 5.404, 8.933], [1, 0.32, 0.12, 0.05], [1, 0.55, 0.3, 0.18]),
    "marimba": ([1, 3.984, 10.668], [1, 0.22, 0.06], [1, 0.25, 0.08]),
    "vibes": ([1, 4.0, 10.0], [1, 0.25, 0.08], [1, 0.4, 0.15]),
    "kalimba": ([1, 5.93, 16.7], [1, 0.18, 0.05], [1, 0.3, 0.1]),
    "bell": ([0.5, 1, 1.183, 1.506, 2, 2.514, 2.662, 3.011], [0.6, 1, 0.45, 0.3, 0.42, 0.2, 0.18, 0.12],
             [1.6, 1, 0.8, 0.6, 0.5, 0.4, 0.35, 0.3]),
    "steel": ([1, 2, 3.0, 3.94], [1, 0.5, 0.18, 0.08], [1, 0.6, 0.35, 0.2]),
    "metal": ([1, 1.59, 2.14, 2.65, 3.17, 3.9, 5.3], [1, 0.8, 0.65, 0.5, 0.42, 0.3, 0.2],
              [1, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4]),
    "wood": ([1, 2.57, 4.4], [1, 0.3, 0.1], [1, 0.3, 0.12]),
]

final class ModalVoice: Voice {
    var partials: [Resonator] = []
    var strike: Float
    var strikeFilter = SVF()
    let tremolo: Float
    let damp: Float
    var dampGain: Float = 1
    let dampCoef: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        let material = materials[p.s("material", "glass")] ?? materials["glass"]!
        let decay = p.f("decay", 2.0) * (1 - 0.4 * clamp01((event.note - 72) / 36))
        let brightness = p.f("brightness", 0.6)
        strike = p.f("strike", 0.25)
        tremolo = p.f("tremolo", 0)
        damp = p.f("damp", 0)
        dampCoef = expf(-1 / max(1, p.f("release", 0.4) * sampleRate / 6.9))
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        let f = midiToHz(event.note)
        var r = Rng(seed: hash64(seed, UInt64(event.index), 991))
        for k in 0..<material.ratios.count {
            let ratio = material.ratios[k] * (1 + r.bipolar() * 0.002)
            let tilt = powf(Float(k + 1), -(1.2 - brightness))
            partials.append(Resonator(frequency: f * ratio, decay: decay * material.decays[k],
                                      amplitude: material.amps[k] * tilt * (0.4 + 0.6 * velocity),
                                      phase: r.unit() * twoPi, sampleRate: sampleRate))
        }
        strikeFilter.set(cutoff: min(9000, f * 4), q: 1.2, sampleRate: sampleRate)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        var energy: Float = 0
        for i in 0..<count {
            var s: Float = 0
            for k in 0..<partials.count { s += partials[k].next() }
            if elapsed < 400 {
                let n = rng.bipolar() * strike * velocity * expf(-Float(elapsed) / 60)
                s += strikeFilter.bandpass(n)
            }
            if damp > 0 && elapsed >= gateSamples { dampGain *= dampCoef }
            var g: Float = 0.18 * dampGain
            if tremolo > 0 { g *= 1 - tremolo * (0.5 + 0.5 * sinf(twoPi * 5.5 * (control.time + Float(i) / sampleRate))) }
            l[i] += s * g * panL * 1.414
            r[i] += s * g * panR * 1.414
            elapsed += 1
        }
        for k in 0..<partials.count { energy += partials[k].energy }
        return (energy * dampGain > 1e-9) || elapsed < 2000
    }
}

// MARK: - plucked string (Karplus–Strong)

final class PluckVoice: Voice {
    let line: DelayLine
    let period: Float
    var loopGain: Float
    let brightness: Float
    var last: Float = 0
    var body = SVF()
    let bodyMix: Float
    var excitation = 0
    let excitationLength: Int
    var excite = OnePole()
    var muteCoef: Float = 1
    var level: Float = 1
    let muted: Bool
    var dc = OnePole()
    var silentRun = 0
    var tone = OnePole()
    let subAmount: Float
    var sub = Osc()
    let subDt: Float
    var subLevel: Float = 1
    let subDecay: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        let f = midiToHz(event.note)
        period = sampleRate / f
        line = DelayLine(maxSamples: Int(period) + 4)
        let decay = p.f("decay", 2.5) * (1 - 0.5 * clamp01((event.note - 60) / 36))
        muted = p.f("mute", 0) > 0.5
        loopGain = powf(10, -3 / (max(0.05, muted ? decay * 0.08 : decay) * f))
        brightness = clamp01(p.f("brightness", 0.55) * (0.6 + 0.4 * event.velocity))
        bodyMix = p.f("body", 0.3)
        excitationLength = Int(period)
        subAmount = p.f("sub", 0)
        subDt = f / sampleRate
        subDecay = expf(-1 / max(1, decay * 0.6 * sampleRate / 6.9))
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        excite.setLowpass(cutoff: 600 + 9000 * brightness, sampleRate: sampleRate)
        tone.setLowpass(cutoff: p.f("lowpass", 14000), sampleRate: sampleRate)
        body.set(cutoff: p.f("body_hz", 190), q: 1.4, sampleRate: sampleRate)
        muteCoef = expf(-1 / max(1, p.f("release", 0.08) * sampleRate / 6.9))
        dc.setLowpass(cutoff: 20, sampleRate: sampleRate)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let blend = 0.5 - 0.45 * brightness * 0.5
        var peak: Float = 0
        for i in 0..<count {
            var input: Float = 0
            if excitation < excitationLength {
                input = excite.lowpass(rng.bipolar()) * velocity
                excitation += 1
            }
            let delayed = line.read(period - 0.5)
            let y = loopGain * ((1 - blend) * delayed + blend * last)
            last = delayed
            line.push(input + y)
            if elapsed >= gateSamples { level *= muteCoef }
            var s = tone.lowpass(dc.highpass(y)) * level
            s += body.bandpass(s) * bodyMix
            if subAmount > 0 {
                // A sine at the fundamental: the round weight a plucked bass needs.
                subLevel *= elapsed >= gateSamples ? muteCoef : subDecay
                s += sub.sine(subDt) * subAmount * subLevel * velocity * min(1, Float(elapsed) / 48)
            }
            l[i] += s * 0.5 * panL * 1.414
            r[i] += s * 0.5 * panR * 1.414
            peak = max(peak, abs(s))
            elapsed += 1
        }
        silentRun = peak < 1e-5 ? silentRun + count : 0
        return elapsed < excitationLength + 512 || silentRun < 4800
    }
}

// MARK: - bass

final class BassVoice: Voice {
    let mode: String
    var osc1 = Osc(), osc2 = Osc(), osc3 = Osc(), sub = Osc()
    var filter = Ladder()
    var amp: ADSR
    let cutoff: Float, resonance: Float, envAmount: Float, filterDecay: Float, drive: Float
    let detune: Float
    let lfoRate: Float
    var hp = SVF()
    /// Two-band bass (`top` 0…1): a saturated saw band-passed around `top_hz`
    /// rides over the clean low end, so the line reads on phone speakers
    /// without widening or muddying the sub. Off by default.
    let top: Float, topHz: Float, topDrive: Float
    var topOsc = Osc(), topLow = SVF(), topHigh = SVF()

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        mode = p.s("mode", "round")
        let defaults: (Float, Float, Float, Float, Float) = {
            switch p.s("mode", "round") {
            case "reese": return (420, 0.2, 1.4, 0.6, 0.35)
            case "808": return (900, 0.0, 0.0, 0.3, 0.25)
            case "acid": return (380, 0.82, 3.2, 0.18, 0.4)
            case "sub": return (300, 0.0, 0.3, 0.2, 0.05)
            case "chip": return (8000, 0.0, 0.0, 0.2, 0.0)
            default: return (520, 0.28, 2.1, 0.14, 0.3)
            }
        }()
        cutoff = p.f("cutoff", defaults.0)
        resonance = p.f("resonance", defaults.1)
        envAmount = p.f("env_amount", defaults.2) * (mode == "acid" ? 0.6 + 0.8 * event.velocity : 1)
        filterDecay = p.f("f_decay", defaults.3)
        drive = p.f("drive", defaults.4)
        detune = p.f("detune", 0.16)
        lfoRate = p.f("lfo_rate", 0.18)
        let release = p.f("release", mode == "808" ? 0.4 : 0.09)
        amp = ADSR(attack: p.f("attack", 0.003), decay: p.f("decay", mode == "808" ? 2.2 : 0.6),
                   sustain: p.f("sustain", mode == "808" ? 0.0 : 0.82), release: release, sampleRate: sampleRate)
        top = p.f("top", 0); topHz = p.f("top_hz", 700); topDrive = p.f("top_drive", 0.7)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        var r = Rng(seed: hash64(seed, UInt64(event.index), 5))
        osc2.phase = r.unit(); osc3.phase = r.unit()
        hp.set(cutoff: 28, q: 0.7, sampleRate: sampleRate)
        topLow.set(cutoff: topHz * 1.8, q: 0.7, sampleRate: sampleRate)
        topHigh.set(cutoff: topHz * 0.45, q: 0.7, sampleRate: sampleRate)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        var i = 0
        while i < count {
            let block = min(16, count - i)
            let t = Float(elapsed) / sampleRate
            var semis = pitch()
            if mode == "808" { semis += 7 * expf(-t / 0.035) }
            let f = midiToHz(semis)
            let dt = f / sampleRate
            let fenv = expf(-t / max(0.01, filterDecay))
            let lfo = sinf(twoPi * lfoRate * (control.time + Float(i) / sampleRate))
            var fc = cutoff * control.cutoff * powf(2, envAmount * fenv) * (0.75 + 0.25 * velocity)
            if mode == "reese" { fc *= powf(2, 0.7 * lfo) }
            filter.set(cutoff: max(40, fc), resonance: resonance, sampleRate: sampleRate)
            for j in 0..<block {
                if elapsed == gateSamples { amp.gateOff() }
                var s: Float
                switch mode {
                case "reese":
                    let d = detune / 12
                    s = 0.5 * osc1.saw(dt * powf(2, d)) + 0.5 * osc2.saw(dt * powf(2, -d)) + 0.35 * osc3.saw(dt)
                    s = filter.process(s * 0.6, drive: drive)
                    s += 0.75 * sub.sine(dt)
                case "808":
                    s = warm(sub.sine(dt) * 1.1, drive * 2) + 0.15 * osc1.triangle(dt * 2) * expf(-t / 0.05)
                case "acid":
                    s = filter.process(0.7 * osc1.saw(dt) + 0.3 * osc2.pulse(dt, width: 0.5), drive: drive)
                case "sub":
                    s = 0.9 * sub.sine(dt) + 0.18 * osc1.triangle(dt)
                    s = filter.process(s, drive: drive)
                case "chip":
                    s = osc1.chipTriangle(dt) * 0.8
                default:
                    // round funk bass: square + saw through a plucky low-pass, sine underneath
                    s = filter.process(0.55 * osc1.pulse(dt, width: 0.42) + 0.45 * osc2.saw(dt * 1.002), drive: drive)
                    s += 0.55 * sub.sine(dt)
                }
                if top > 0 {
                    // Follows the note's own brightness: the 808's pitch drop and the filter envelope.
                    let edge = topHigh.highpass(topLow.lowpass(saturate(topOsc.saw(dt), topDrive) * 2.2))
                    s += edge * top * (0.55 + 0.45 * fenv)
                }
                s = hp.highpass(warm(s, drive * 0.5))
                let a = amp.next() * 0.42 * (0.5 + 0.5 * velocity)
                l[i + j] += s * a * panL * 1.414
                r[i + j] += s * a * panR * 1.414
                elapsed += 1
            }
            i += block
        }
        return !amp.finished
    }
}

// MARK: - leads (saw, pulse, sine, flute, chip)

final class LeadVoice: Voice {
    let wave: String
    var osc = Osc(), osc2 = Osc()
    var filter = Ladder()
    var amp: ADSR
    let cutoff: Float, resonance: Float, envAmount: Float, filterDecay: Float, drive: Float
    let vibratoDepth: Float, vibratoRate: Float, vibratoDelay: Float
    let breath: Float
    var breathFilter = SVF()
    let duty: Float
    let detune: Float
    /// Filter LFO in octaves (lfo_depth), shaped and tempo-set by the manifest.
    let lfoRate: Float, lfoDepth: Float, lfoShape: String, lfoSeed: UInt64
    /// Unison stack for saw/square/pulse (`unison` 2…7 voices `unison_cents` apart,
    /// `spread` across the stereo field, one filter per side), a `layer_wave`
    /// `layer_semi` away after the filter and analog `drift` in cents. All off by default.
    let unison: Int, unisonCents: Float, spread: Float
    var stack: [Osc] = []
    var filterR = Ladder()
    let layerWave: String, layerSemi: Float, layerMix: Float
    var layerOsc = Osc()
    let drift: Float, driftPhase: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        wave = p.s("wave", "saw")
        cutoff = p.f("cutoff", 2400)
        resonance = p.f("resonance", 0.2)
        envAmount = p.f("env_amount", 1.0)
        filterDecay = p.f("f_decay", 0.4)
        drive = p.f("drive", 0.15)
        vibratoDepth = p.f("vibrato", 0.18)
        vibratoRate = p.f("vibrato_rate", 5.3)
        vibratoDelay = p.f("vibrato_delay", 0.28)
        breath = p.f("breath", wave == "flute" ? 0.18 : 0)
        duty = p.f("duty", 0.25)
        detune = p.f("detune", 0.0)
        lfoRate = p.f("lfo_rate", 0); lfoDepth = p.f("lfo_depth", 0); lfoShape = p.s("lfo_shape", "sine")
        lfoSeed = hash64(seed, UInt64(event.index), 93)
        let stacked = ["saw", "square", "pulse"].contains(wave)
        unison = stacked ? max(1, min(7, Int(p.f("unison", 1)))) : 1
        unisonCents = p.f("unison_cents", 14); spread = p.f("spread", 0.7)
        layerWave = p.s("layer_wave", ""); layerSemi = p.f("layer_semi", -12); layerMix = layerWave.isEmpty ? 0 : p.f("layer_mix", 0.3)
        drift = p.f("drift", 0)
        var d = Rng(seed: hash64(seed, UInt64(event.index), 0xD71E))
        driftPhase = d.unit()
        if unison > 1 { stack = (1..<unison).map { _ in Osc(phase: d.unit()) } }
        amp = ADSR(attack: p.f("attack", wave == "flute" ? 0.06 : 0.012), decay: p.f("decay", 0.4),
                   sustain: p.f("sustain", 0.75), release: p.f("release", 0.25), sampleRate: sampleRate)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        breathFilter.set(cutoff: midiToHz(event.note) * 2, q: 2.5, sampleRate: sampleRate)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        var i = 0
        while i < count {
            let block = min(16, count - i)
            let t = Float(elapsed) / sampleRate
            let vib = vibratoDepth * smoothstep((t - vibratoDelay) / 0.4) * sinf(twoPi * vibratoRate * t)
            var semis = pitch() + vib
            if drift > 0 { semis += analogDrift(control.time + Float(i) / sampleRate, 0, cents: drift, phase: driftPhase) }
            let f = midiToHz(semis)
            let dt = f / sampleRate
            let wobble = lfoDepth > 0 ? lfoDepth * lfoValue(lfoRate * (control.time + Float(i) / sampleRate), lfoShape, seed: lfoSeed) : 0
            let fc = cutoff * control.cutoff * powf(2, envAmount * expf(-t / filterDecay) + wobble) * (0.7 + 0.3 * velocity)
            filter.set(cutoff: fc, resonance: resonance, sampleRate: sampleRate)
            if unison > 1 {
                filterR.set(cutoff: fc * 1.01, resonance: resonance, sampleRate: sampleRate)
                let layerDt = f * powf(2, layerSemi / 12) / sampleRate
                let n = Float(unison), norm = 1 / sqrtf(n)
                for j in 0..<block {
                    if elapsed == gateSamples { amp.gateOff() }
                    var sl: Float = 0, sr: Float = 0
                    for k in 0..<unison {
                        let pos = (Float(k) / (n - 1) * 2 - 1)
                        let ratio = powf(2, pos * unisonCents / 1200)
                        let v: Float
                        if k == 0 {
                            v = wave == "saw" ? osc.saw(dt * ratio) : osc.pulse(dt * ratio, width: wave == "pulse" ? 0.5 + 0.3 * sinf(twoPi * 0.7 * (control.time + Float(i + j) / sampleRate)) : 0.5)
                        } else {
                            v = wave == "saw" ? stack[k - 1].saw(dt * ratio) : stack[k - 1].pulse(dt * ratio, width: 0.5)
                        }
                        let p = pos * spread
                        sl += v * (1 - p) * 0.5 * 1.414
                        sr += v * (1 + p) * 0.5 * 1.414
                    }
                    var yl = filter.process(sl * norm, drive: drive), yr = filterR.process(sr * norm, drive: drive)
                    // The layer sits under the filter, like the mono path's.
                    if layerMix > 0 { let s = layerOsc.wave(layerWave, layerDt) * layerMix; yl += s; yr += s }
                    let a = amp.next() * 0.26 * (0.45 + 0.55 * velocity)
                    l[i + j] += yl * a * panL * 1.414
                    r[i + j] += yr * a * panR * 1.414
                    elapsed += 1
                }
                i += block
                continue
            }
            let layerDt = f * powf(2, layerSemi / 12) / sampleRate
            for j in 0..<block {
                if elapsed == gateSamples { amp.gateOff() }
                var s: Float
                switch wave {
                case "pulse":
                    let pw = 0.5 + 0.3 * sinf(twoPi * 0.7 * (control.time + Float(i + j) / sampleRate))
                    s = filter.process(osc.pulse(dt, width: pw), drive: drive)
                case "square": s = filter.process(osc.pulse(dt, width: 0.5), drive: drive)
                case "sine": s = osc.sine(dt) + 0.08 * sinf(4 * .pi * osc.phase)
                case "tri": s = osc.triangle(dt)
                case "chip": s = osc.chipPulse(dt, width: duty) * 0.55
                case "flute":
                    let tone = osc.sine(dt) + 0.12 * sinf(4 * .pi * osc.phase) + 0.05 * sinf(6 * .pi * osc.phase)
                    let air = breathFilter.bandpass(rng.bipolar()) * breath * (1 + 2 * expf(-t / 0.05))
                    s = tone * min(1, t / 0.04) + air * 3
                case "supersaw":
                    s = filter.process(0.5 * osc.saw(dt * powf(2, detune / 1200)) + 0.5 * osc2.saw(dt * powf(2, -detune / 1200)), drive: drive)
                default: s = filter.process(osc.saw(dt), drive: drive)
                }
                if layerMix > 0 { s += layerOsc.wave(layerWave, layerDt) * layerMix }
                let a = amp.next() * 0.26 * (0.45 + 0.55 * velocity)
                l[i + j] += s * a * panL * 1.414
                r[i + j] += s * a * panR * 1.414
                elapsed += 1
            }
            i += block
        }
        return !amp.finished
    }
}

// MARK: - textures (long, evolving beds)

/// One granular wavelet: sine plus a little second and third harmonic under a
/// Hann window, or a swelling (reversed) window that cuts off at its peak.
struct Grain {
    var phase: Float, step: Float, age: Float, length: Float, amplitude: Float
    let reverse: Bool, harmonic: Float, panL: Float, panR: Float
    var done: Bool { age >= length }
    @inline(__always) mutating func next() -> Float {
        if age >= length { return 0 }
        let u = age / length
        let window = reverse ? powf(u, 2.2) * (u < 0.97 ? 1 : (1 - u) / 0.03) : 0.5 - 0.5 * cosf(twoPi * u)
        let s = sinf(twoPi * phase) + harmonic * (0.5 * sinf(2 * twoPi * phase) + 0.25 * sinf(3 * twoPi * phase))
        phase += step; if phase >= 1 { phase -= floorf(phase) }
        age += 1
        return s * window * amplitude
    }
}

final class TextureVoice: Voice {
    let kind: String
    var level: Float = 0
    let attack: Float, release: Float
    var resonators: [Osc] = []
    var ratios: [Float] = []
    var phases: [Float] = []
    var filterL = SVF(), filterR = SVF()
    var pinkL = PinkNoise(), pinkR = PinkNoise()
    let density: Float
    let notes: [Float]
    var grains: [Grain] = []
    /// Revision 1 grains: decaying resonators at the chord tones, with their pans.
    var resonatorGrains: [Resonator] = []
    var resonatorPans: [(Float, Float)] = []
    /// Granular: grain length (s) and spray; 'cloud' uses long overlapping grains.
    let grainLength: Float, spray: Float
    let brightness: Float
    var bitHold: (Float, Float) = (0, 0)
    var holdCount = 0
    var tone = Osc()
    var popL: Float = 0, popR: Float = 0, popCoef: Float = 0.99
    var popToneL = OnePole(), popToneR = OnePole()
    var toneHz: Float = 0
    var toneLeft = 0

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        kind = p.s("kind", "drone")
        attack = p.f("attack", 3.0)
        release = p.f("release", 4.0)
        density = p.f("density", 2.0)
        notes = p.list("notes")
        brightness = p.f("brightness", 0.5)
        grainLength = p.f("grain", kind == "cloud" ? 0.7 : 0.14)
        spray = p.f("spray", 0.5)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        popToneL.setLowpass(cutoff: 3200, sampleRate: sampleRate)
        popToneR.setLowpass(cutoff: 3000, sampleRate: sampleRate)
        if kind == "drone" {
            ratios = p.list("ratios").isEmpty ? [1, 1.5, 2, 3, 4.01] : p.list("ratios")
            resonators = ratios.map { _ in Osc(phase: rng.unit()) }
            phases = ratios.map { _ in rng.unit() }
        }
    }

    @inline(__always) func envelope() -> Float {
        let t = Float(elapsed) / sampleRate
        let gate = Float(gateSamples) / sampleRate
        let fadeIn = smoothstep(t / max(0.01, attack))
        let fadeOut = t <= gate ? 1 : 1 - smoothstep((t - gate) / max(0.01, release))
        return fadeIn * fadeOut
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let total = gateSamples + Int(release * sampleRate)
        let base = midiToHz(note)
        var i = 0
        while i < count {
            let block = min(32, count - i)
            let t = control.time + Float(i) / sampleRate
            let env = envelope()
            switch kind {
            case "air":
                let center = 900 * powf(2, 2.2 * (0.5 + 0.5 * sinf(twoPi * (0.05 * t + phase(0)))) * brightness + 0.3)
                filterL.set(cutoff: center, q: 1.6, sampleRate: sampleRate)
                filterR.set(cutoff: center * 1.18, q: 1.6, sampleRate: sampleRate)
            case "drone":
                filterL.set(cutoff: base * (3 + 5 * brightness * (0.5 + 0.5 * sinf(twoPi * 0.031 * t))), q: 3, sampleRate: sampleRate)
                filterR.set(cutoff: base * (3.2 + 5 * brightness * (0.5 + 0.5 * sinf(twoPi * 0.027 * t + 1))), q: 3, sampleRate: sampleRate)
            case "hiss":
                filterL.set(cutoff: 7000, q: 0.6, sampleRate: sampleRate)
                filterR.set(cutoff: 7200, q: 0.6, sampleRate: sampleRate)
            case "crackle":
                filterL.set(cutoff: 4200, q: 0.6, sampleRate: sampleRate)
                filterR.set(cutoff: 4400, q: 0.6, sampleRate: sampleRate)
            case "data":
                filterL.set(cutoff: 3200, q: 0.8, sampleRate: sampleRate)
                filterR.set(cutoff: 3300, q: 0.8, sampleRate: sampleRate)
            case "insects":
                filterL.set(cutoff: 4800 + 1400 * phase(1), q: 7, sampleRate: sampleRate)
                filterR.set(cutoff: 5100 + 1400 * phase(2), q: 7, sampleRate: sampleRate)
            default: break
            }
            for j in 0..<block {
                var sl: Float = 0, sr: Float = 0
                let localT = t + Float(j) / sampleRate
                switch kind {
                case "drone":
                    for k in 0..<resonators.count {
                        let wobble = 0.55 + 0.45 * sinf(twoPi * (0.07 + 0.05 * Float(k)) * localT + phases[k] * twoPi)
                        let s = resonators[k].sine(base * ratios[k] / sampleRate) * wobble / Float(k + 1)
                        if k % 2 == 0 { sl += s; sr += s * 0.7 } else { sr += s; sl += s * 0.7 }
                    }
                    sl = sl * 0.5 + filterL.bandpass(pinkL.process(rng.bipolar())) * 0.9
                    sr = sr * 0.5 + filterR.bandpass(pinkR.process(rng.bipolar())) * 0.9
                    sl *= 0.16; sr *= 0.16
                case "air":
                    sl = filterL.bandpass(pinkL.process(rng.bipolar())) * 0.5
                    sr = filterR.bandpass(pinkR.process(rng.bipolar())) * 0.5
                case "hiss":
                    sl = filterL.lowpass(rng.bipolar()) * 0.05
                    sr = filterR.lowpass(rng.bipolar()) * 0.05
                case "crackle":
                    // Vinyl: a dark surface hiss plus pops that are tiny bursts of
                    // filtered noise with a soft decay (never single-sample clicks).
                    sl = filterL.lowpass(pinkL.process(rng.bipolar())) * 0.02
                    sr = filterR.lowpass(pinkR.process(rng.bipolar())) * 0.02
                    if rng.unit() < density * 2 / sampleRate {
                        let size = rng.unit()
                        let level = 0.012 + 0.05 * size * size * size
                        let side = rng.bipolar()
                        popL = max(popL, level * (1 - max(0, side) * 0.7))
                        popR = max(popR, level * (1 + min(0, side) * 0.7))
                        popCoef = expf(-1 / ((0.0008 + 0.0025 * size) * sampleRate))
                    }
                    if popL > 1e-4 || popR > 1e-4 {
                        let n = rng.bipolar()
                        sl += popToneL.lowpass(n * popL)
                        sr += popToneR.lowpass(n * popR * 0.9 + rng.bipolar() * popR * 0.1)
                        popL *= popCoef
                        popR *= popCoef
                    }
                case "grains" where revision == .v1:
                    if !notes.isEmpty && rng.unit() < density / sampleRate {
                        let pick = notes[Int(rng.unit() * Float(notes.count)) % notes.count]
                        let octave: Float = rng.unit() < 0.3 ? 12 : 0
                        resonatorGrains.append(Resonator(frequency: midiToHz(pick + octave), decay: 0.6 + 1.6 * rng.unit(),
                                                         amplitude: 0.25 + 0.35 * rng.unit(), phase: 0, sampleRate: sampleRate))
                        let pos = rng.bipolar() * 0.85
                        resonatorPans.append((cosf((pos + 1) * .pi / 4), sinf((pos + 1) * .pi / 4)))
                    }
                    var k = 0
                    while k < resonatorGrains.count {
                        let s = resonatorGrains[k].next() * 0.2
                        sl += s * resonatorPans[k].0
                        sr += s * resonatorPans[k].1
                        k += 1
                    }
                case "grains", "cloud":
                    // Windowed grains of the chord tones: octave/fifth leaps, a few
                    // cents of spray, reversed (swelling) grains, wide stereo. 'cloud'
                    // overlaps long grains into a shimmering bed.
                    if !notes.isEmpty && rng.unit() < density / sampleRate {
                        let pick = notes[Int(rng.unit() * Float(notes.count)) % notes.count]
                        let leaps: [Float] = kind == "cloud" ? [0, 0, 12, 19, 24] : [0, 12, 12, 19, 24, -12]
                        let leap = leaps[Int(rng.unit() * Float(leaps.count)) % leaps.count]
                        let cents = rng.bipolar() * 9 * spray
                        let length = grainLength * (0.6 + 0.9 * rng.unit())
                        let pos = rng.bipolar() * (0.4 + 0.55 * spray)
                        grains.append(Grain(phase: rng.unit(), step: midiToHz(pick + leap + cents / 100) / sampleRate,
                                            age: 0, length: max(0.02, length) * sampleRate, amplitude: 0.35 + 0.45 * rng.unit(),
                                            reverse: rng.unit() < 0.3, harmonic: brightness * (0.3 + 0.5 * rng.unit()),
                                            panL: cosf((pos + 1) * .pi / 4), panR: sinf((pos + 1) * .pi / 4)))
                    }
                    var k = 0
                    while k < grains.count {
                        let s = grains[k].next() * (kind == "cloud" ? 0.12 : 0.2)
                        sl += s * grains[k].panL
                        sr += s * grains[k].panR
                        k += 1
                    }
                case "insects":
                    // Cicadas: a narrow noise band amplitude-modulated at insect rate.
                    let am = 0.5 + 0.5 * sinf(twoPi * (36 + 12 * phase(3)) * localT)
                    sl = filterL.bandpass(rng.bipolar()) * am * 0.22
                    sr = filterR.bandpass(rng.bipolar()) * (1 - am * 0.6) * 0.18
                case "data":
                    // Modem-ish chatter: short bursts of tones from the scale, crushed.
                    if toneLeft <= 0 {
                        if rng.unit() < 0.35 && !notes.isEmpty {
                            toneHz = midiToHz(notes[Int(rng.unit() * Float(notes.count)) % notes.count] + 24)
                        } else {
                            toneHz = rng.unit() < 0.5 ? 0 : 1200 + 1200 * rng.unit()
                        }
                        toneLeft = Int(sampleRate * (rng.unit() < 0.7 ? 0.035 : 0.12))
                    }
                    toneLeft -= 1
                    let s = toneHz > 0 ? tone.pulse(toneHz / sampleRate, width: 0.5) * 0.09 : 0
                    holdCount -= 1
                    if holdCount <= 0 { bitHold = (floorf(s * 12) / 12, floorf(s * 12) / 12); holdCount = 4 }
                    sl = filterL.bandpass(bitHold.0) * 1.4 + rng.bipolar() * 0.004
                    sr = filterR.bandpass(bitHold.1) * 1.4 + rng.bipolar() * 0.004
                default: break
                }
                let a = env * (0.5 + 0.5 * velocity)
                l[i + j] += sl * a
                r[i + j] += sr * a
                elapsed += 1
            }
            if !grains.isEmpty { grains.removeAll { $0.done } }
            if !resonatorGrains.isEmpty {
                var keep: [Resonator] = [], keepPans: [(Float, Float)] = []
                for (k, g) in resonatorGrains.enumerated() where g.energy > 1e-8 { keep.append(g); keepPans.append(resonatorPans[k]) }
                resonatorGrains = keep; resonatorPans = keepPans
            }
            i += block
        }
        return elapsed < total || !grains.isEmpty
    }

    private func phase(_ k: Int) -> Float { Float(hash64(UInt64(k), UInt64(note * 10)) % 1000) / 1000 }
}

// MARK: - transition effects

final class FxVoice: Voice {
    let kind: String
    var osc = Osc(), osc2 = Osc()
    var metal: [Resonator] = []
    var shepardPhases = [Float](repeating: 0, count: 6)
    var filterL = SVF(), filterR = SVF()
    var pinkL = PinkNoise(), pinkR = PinkNoise()
    let length: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        kind = spec.params.s("kind", "riser")
        length = Float(event.duration)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let impactTail: Float = revision == .v1 ? 3.0 : 3.5
        let tail: Float = kind == "impact" ? impactTail : kind == "subdrop" ? 1.6 : kind == "downlifter" ? 0.5 : 0.02
        let total = Int((length + tail) * sampleRate)
        var i = 0
        while i < count {
            let block = min(16, count - i)
            let t = Float(elapsed) / sampleRate
            let u = clamp01(t / max(0.05, length))
            switch kind {
            case "riser":
                let fc = 250 * powf(2, 5.2 * u * u)
                filterL.set(cutoff: fc, q: 2.2, sampleRate: sampleRate)
                filterR.set(cutoff: fc * 1.05, q: 2.2, sampleRate: sampleRate)
            case "downlifter":
                let fc = 9000 * powf(2, -5 * u)
                filterL.set(cutoff: fc, q: 1.2, sampleRate: sampleRate)
                filterR.set(cutoff: fc * 0.97, q: 1.2, sampleRate: sampleRate)
            case "swell":
                filterL.set(cutoff: 2600 + 5000 * u, q: 0.7, sampleRate: sampleRate)
                filterR.set(cutoff: 2700 + 5000 * u, q: 0.7, sampleRate: sampleRate)
            case "impact":
                filterL.set(cutoff: 900 * expf(-t * 3) + 120, q: 0.8, sampleRate: sampleRate)
                filterR.set(cutoff: 950 * expf(-t * 3) + 120, q: 0.8, sampleRate: sampleRate)
            case "reverse":
                // Reverse cymbal: the band falls as the swell rises into the hit.
                let fc = 12000 * powf(2, -2.2 * (1 - u))
                filterL.set(cutoff: fc, q: 0.6, sampleRate: sampleRate)
                filterR.set(cutoff: fc * 0.96, q: 0.6, sampleRate: sampleRate)
            case "scratch":
                let fc = 900 + 2600 * (0.5 + 0.5 * sinf(twoPi * 9 * t))
                filterL.set(cutoff: fc, q: 2.5, sampleRate: sampleRate)
                filterR.set(cutoff: fc * 1.05, q: 2.5, sampleRate: sampleRate)
            default:
                filterL.set(cutoff: 400 + 6000 * u, q: 3, sampleRate: sampleRate)
                filterR.set(cutoff: 420 + 6000 * u, q: 3, sampleRate: sampleRate)
            }
            for j in 0..<block {
                let tt = Float(elapsed) / sampleRate
                let uu = clamp01(tt / max(0.05, length))
                var sl: Float = 0, sr: Float = 0
                let nl = pinkL.process(rng.bipolar()), nr = pinkR.process(rng.bipolar())
                switch kind {
                case "riser":
                    let env = uu * uu * uu * (tt < length ? 1 : 0)
                    let tone = osc.saw(midiToHz(note + 12 * uu * uu) / sampleRate) * 0.25
                    sl = (filterL.bandpass(nl) * 1.8 + tone) * env
                    sr = (filterR.bandpass(nr) * 1.8 + tone) * env
                case "downlifter":
                    let env = (1 - uu) * (1 - uu)
                    let tone = osc.sine(midiToHz(note - 14 * uu) / sampleRate) * 0.4
                    sl = (filterL.lowpass(nl) + tone) * env
                    sr = (filterR.lowpass(nr) + tone) * env
                case "swell":
                    let env = powf(uu, 2.6) * (tt < length ? 1 : max(0, 1 - (tt - length) / 0.02))
                    sl = filterL.highpass(nl) * env * 1.4
                    sr = filterR.highpass(nr) * env * 1.4
                case "impact" where revision == .v1:
                    let boom = warm(osc.sine(midiToHz(note + 12 * expf(-tt / 0.06)) / sampleRate), 0.6) * expf(-tt / 0.9)
                    let crack = filterL.lowpass(nl) * expf(-tt / 0.18)
                    sl = boom * 0.9 + crack * 1.4
                    sr = boom * 0.9 + filterR.lowpass(nr) * expf(-tt / 0.18) * 1.4
                case "impact":
                    // Boom (pitch-dropping sub), a noise crack and a metallic ring.
                    let boom = warm(osc.sine(midiToHz(note + 14 * expf(-tt / 0.05)) / sampleRate), 0.8) * expf(-tt / 1.1)
                    if metal.isEmpty {
                        var r2 = Rng(seed: UInt64(note * 100) &+ 17)
                        metal = [1, 1.59, 2.14, 2.92, 3.71].map { Resonator(frequency: midiToHz(note + 36) * $0 * (1 + r2.bipolar() * 0.01), decay: 1.6, amplitude: 0.12, phase: r2.unit() * twoPi, sampleRate: sampleRate) }
                    }
                    var ring: Float = 0
                    for k in 0..<metal.count { ring += metal[k].next() }
                    // The crack is mono so the low end stays solid on phones.
                    let crack = filterL.lowpass(nl) * expf(-tt / 0.16)
                    sl = boom * 1.1 + crack * 1.5 + ring * 0.6
                    sr = boom * 1.1 + crack * 1.5 + ring * 0.55
                case "subdrop":
                    let f = midiToHz(note + 19 * expf(-tt / 0.16))
                    let s = warm(osc.sine(f / sampleRate), 0.5) * min(1, tt / 0.004) * expf(-tt / 0.7)
                    sl = s * 1.2; sr = s * 1.2
                case "reverse":
                    let env = powf(uu, 3.2) * (tt < length ? 1 : max(0, 1 - (tt - length) / 0.012))
                    let shimmer = (osc.pulse(5400 / sampleRate, width: 0.5) + osc2.pulse(7900 / sampleRate, width: 0.5)) * 0.08
                    sl = (filterL.highpass(nl * 1.4) + shimmer) * env * 1.3
                    sr = (filterR.highpass(nr * 1.4) + shimmer) * env * 1.3
                case "shepard":
                    // Endless rise: octave-spaced voices climbing under a fixed bell curve.
                    var s: Float = 0
                    for k in 0..<6 {
                        let position = (Float(k) + uu * 1.5).truncatingRemainder(dividingBy: 6) / 6
                        let hz = midiToHz(note - 12) * powf(2, position * 6)
                        shepardPhases[k] += hz / sampleRate
                        if shepardPhases[k] >= 1 { shepardPhases[k] -= floorf(shepardPhases[k]) }
                        let weight = expf(-powf((position - 0.5) / 0.18, 2))
                        s += (sinf(twoPi * shepardPhases[k]) + 0.3 * sinf(4 * .pi * shepardPhases[k])) * weight
                    }
                    let env = smoothstep(uu / 0.3) * (tt < length ? 1 : max(0, 1 - (tt - length) / 0.02))
                    sl = s * env * 0.32; sr = s * env * 0.3
                case "scratch":
                    let env = sinf(.pi * uu)
                    sl = filterL.bandpass(nl * 2) * env * 1.4
                    sr = filterR.bandpass(nr * 2) * env * 1.4
                default:
                    let env = sinf(.pi * uu)
                    sl = filterL.bandpass(nl) * env
                    sr = filterR.bandpass(nr) * env
                }
                let a: Float = 0.5 * velocity
                l[i + j] += sl * a * panL * 1.414
                r[i + j] += sr * a * panR * 1.414
                elapsed += 1
            }
            i += block
        }
        return elapsed < total
    }
}
