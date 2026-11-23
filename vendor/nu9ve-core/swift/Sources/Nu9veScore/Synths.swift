import Foundation

// Melodic voices for the film scores: FM (bells, keys, agogô, DX), guitar
// strings into an amp insert, a modal piano, ensemble/spiccato strings,
// brass, the trailer braam, growl/wobble basses and the amapiano log drum.

// MARK: - two-operator FM

/// Bells, Rhodes-like keys, agogô and FM basses. The index decays faster
/// than the amplitude so attacks are bright and tails pure.
final class FMVoice: Voice {
    var carrier: Float = 0, carrier2: Float = 0, modulator: Float = 0
    let ratio: Float, index: Float, indexT60: Float, attack: Float, detune: Float, feedback: Float, drive: Float
    var level: Float = 0
    var decayCoef: Float = 1, releaseCoef: Float = 1
    var lp = SVF(), lpR = SVF()
    var last: Float = 0
    let floorIndex: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        ratio = p.f("ratio", 1)
        index = p.f("index", 1.5)
        indexT60 = p.f("index_t60", 0.3)
        attack = p.f("attack", 0.003)
        detune = p.f("detune", 0)
        feedback = p.f("feedback", 0)
        drive = p.f("drive", 0)
        floorIndex = p.f("index_floor", 0)
        let t60 = p.f("t60", 1.2) * (1 - 0.3 * clamp01((event.note - 72) / 30))
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        decayCoef = expf(-6.91 / max(1, t60 * sampleRate))
        releaseCoef = expf(-6.91 / max(1, p.f("release", 0.2) * sampleRate))
        let cutoff = p.f("lowpass", 9000) * (0.6 + 0.4 * velocity)
        lp.set(cutoff: cutoff, q: 0.6, sampleRate: sampleRate)
        lpR.set(cutoff: cutoff * 1.03, q: 0.6, sampleRate: sampleRate)
        carrier = rng.unit(); carrier2 = rng.unit()
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let sr = sampleRate
        var i = 0
        while i < count {
            let block = min(16, count - i)
            let f = midiToHz(pitch())
            let dt = f / sr
            for j in 0..<block {
                let t = Float(elapsed) / sr
                let a: Float
                if t < attack { level = t / max(1e-4, attack) } else { level *= elapsed >= gateSamples ? decayCoef * releaseCoef : decayCoef }
                a = level
                let idx = (index * (0.35 + 0.65 * velocity) * expf(-6.91 * t / max(0.01, indexT60)) + floorIndex)
                let m = sinf(twoPi * modulator + feedback * last) * idx
                last = m
                var sl = sinf(twoPi * carrier + m)
                var srr = detune > 0 ? sinf(twoPi * carrier2 + m) : sl
                if drive > 0 { sl = warm(sl, drive); srr = warm(srr, drive) }
                sl = lp.lowpass(sl); srr = lpR.lowpass(srr)
                let g = a * 0.3 * (0.4 + 0.6 * velocity)
                l[i + j] += sl * g * panL * 1.414
                r[i + j] += srr * g * panR * 1.414
                carrier += dt; if carrier >= 1 { carrier -= 1 }
                carrier2 += dt * powf(2, detune / 1200); if carrier2 >= 1 { carrier2 -= 1 }
                modulator += dt * ratio; if modulator >= 1 { modulator -= floorf(modulator) }
                elapsed += 1
            }
            i += block
        }
        return level > 1e-5 || Float(elapsed) / sr < attack + 0.01
    }
}

// MARK: - guitar strings (the amp lives on the instrument insert)

final class GuitarVoice: Voice {
    let wave: String
    let articulation: String
    var osc = Osc(), osc2 = Osc()
    var filter = SVF()
    let detuneRatio: Float
    var level: Float = 0
    let startCutoff: Float, endCutoff: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        wave = p.s("wave", "saw")
        articulation = p.s("articulation", "open")
        var r = Rng(seed: hash64(seed, UInt64(event.index), 0x6017))
        detuneRatio = powf(2, r.bipolar() * 6 / 1200)
        let bright = p.f("brightness", 0.6)
        switch articulation {
        case "mute": startCutoff = 900 + 400 * bright; endCutoff = 520
        case "pick": startCutoff = 3800; endCutoff = 1400
        default: startCutoff = (wave == "saw" ? 5600 : 3600) * (0.7 + 0.5 * bright); endCutoff = startCutoff * 0.6
        }
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        osc.phase = r.unit(); osc2.phase = r.unit()
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let sr = sampleRate
        let mute = articulation == "mute", pick = articulation == "pick"
        var i = 0
        var alive = true
        while i < count {
            let block = min(16, count - i)
            let t = Float(elapsed) / sr
            let fc = endCutoff + (startCutoff - endCutoff) * expf(-t / (mute ? 0.02 : pick ? 0.12 : 0.4))
            filter.set(cutoff: fc * control.cutoff, q: mute ? 1.4 : 0.75, sampleRate: sr)
            let f = midiToHz(pitch())
            for j in 0..<block {
                let tt = Float(elapsed) / sr
                let target: Float
                if mute { target = tt < 0.002 ? tt / 0.002 : expf(-(tt - 0.002) / 0.03) }
                else if pick { target = tt < 0.002 ? tt / 0.002 : elapsed < gateSamples ? expf(-(tt - 0.002) / 0.35) : 0 }
                else { target = tt < 0.004 ? tt / 0.004 : elapsed < gateSamples ? 0.7 + 0.3 * expf(-(tt - 0.004) / 0.25) : 0 }
                // Releases glide down instead of clicking.
                level += (target - level) * (target < level && elapsed >= gateSamples ? 0.0009 : 0.35)
                let s: Float
                switch wave {
                case "square": s = osc.pulse(f / sr, width: 0.5) * 0.7 + osc2.triangle(f * 2 / sr) * 0.15
                case "tri": s = osc.triangle(f / sr) + 0.2 * osc2.saw(f * detuneRatio / sr)
                default: s = 0.6 * osc.saw(f / sr) + 0.4 * osc2.saw(f * detuneRatio / sr)
                }
                let y = filter.lowpass(s) * level * 0.32 * (0.5 + 0.5 * velocity)
                l[i + j] += y * panL * 1.414
                r[i + j] += y * panR * 1.414
                elapsed += 1
            }
            i += block
            if (mute && t > 0.2) || (elapsed > gateSamples + Int(0.4 * sr) && level < 1e-4) { alive = false }
        }
        return alive
    }
}

// MARK: - modal piano

/// Stretched-partial piano: two slightly detuned strings per partial (the
/// beating that makes it live), hammer noise, damper on release.
final class PianoVoice: Voice {
    var strings: [Resonator] = []
    var hammer = SVF()
    var damper: Float = 1
    let damperCoef: Float
    let level: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        let f = midiToHz(event.note)
        // Bass strings ring for seconds; the top octave is a short ping.
        let base = p.f("decay", 1) * 7 * powf(2, -(event.note - 33) / 22)
        let inharmonic: Float = 0.00035
        let bright = clamp01(p.f("brightness", 0.6) * (0.5 + 0.7 * event.velocity))
        level = p.f("level", 1)
        damperCoef = expf(-6.91 / max(1, p.f("release", 0.18) * sampleRate))
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        var n: Float = 1
        while n <= 14 && f * n < 11000 {
            let fn = f * n * sqrtf(1 + inharmonic * n * n)
            let amp = powf(n, -(1.55 - bright)) * (n == 1 ? 1 : 0.9)
            let decay = max(0.08, base / (1 + 0.32 * (n - 1)))
            for side: Float in [-1, 1] {
                strings.append(Resonator(frequency: fn * (1 + side * 0.00035 * (1 + n * 0.1)), decay: decay,
                                         amplitude: amp * 0.5, phase: rng.unit() * twoPi, sampleRate: sampleRate))
            }
            n += 1
        }
        hammer.set(cutoff: 1800 + 6000 * bright, q: 0.7, sampleRate: sampleRate)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        var energy: Float = 0
        for i in 0..<count {
            var a: Float = 0, b: Float = 0
            for k in 0..<strings.count {
                let v = strings[k].next()
                if k % 2 == 0 { a += v } else { b += v }
            }
            if elapsed < 300 {
                let n = hammer.lowpass(rng.bipolar()) * velocity * expf(-Float(elapsed) / 70) * 0.6
                a += n; b += n
            }
            if elapsed >= gateSamples { damper *= damperCoef }
            let g = 0.16 * damper * level * (0.35 + 0.65 * velocity)
            // Unison strings spread slightly across the stereo field.
            l[i] += (a * 0.62 + b * 0.38) * g * panL * 1.414
            r[i] += (a * 0.38 + b * 0.62) * g * panR * 1.414
            elapsed += 1
        }
        for k in 0..<strings.count { energy += strings[k].energy }
        return energy * damper > 1e-9 || elapsed < 600
    }
}

// MARK: - strings (ensemble and spiccato)

final class StringsVoice: Voice {
    var oscs: [Osc]
    let detunes: [Float]
    let drifts: [Float]
    let pans: [(Float, Float)]
    var filterL = Ladder(), filterR = Ladder()
    var bodyL = SVF(), bodyR = SVF(), bowL = SVF(), bowR = SVF()
    var amp: ADSR
    let attack: Float, spiccato: Bool, brightness: Float, vibrato: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        spiccato = p.f("spiccato", 0) > 0.5
        let count = spiccato ? 3 : max(2, min(7, Int(p.f("unison", 6))))
        var r = Rng(seed: hash64(seed, UInt64(event.index), 0x5715))
        oscs = (0..<count).map { _ in Osc(phase: r.unit()) }
        let cents = p.f("detune", spiccato ? 8 : 13)
        detunes = (0..<count).map { k in count == 1 ? 0 : (Float(k) / Float(count - 1) * 2 - 1) * cents / 1200 }
        drifts = (0..<count).map { _ in 0.15 + r.unit() * 0.5 }
        pans = (0..<count).map { k in
            let pos = count == 1 ? 0 : (Float(k) / Float(count - 1) * 2 - 1) * 0.85
            return (cosf((pos + 1) * .pi / 4), sinf((pos + 1) * .pi / 4))
        }
        brightness = p.f("brightness", 0.5)
        vibrato = p.f("vibrato", 0.09)
        attack = spiccato ? 0.004 : p.f("attack", 0.35)
        amp = ADSR(attack: attack, decay: spiccato ? 0.11 : 1.5, sustain: spiccato ? 0 : 0.85,
                   release: spiccato ? 0.12 : p.f("release", 0.7), sampleRate: sampleRate)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        bodyL.set(cutoff: 290, q: 1.3, sampleRate: sampleRate); bodyR.set(cutoff: 305, q: 1.3, sampleRate: sampleRate)
        bowL.set(cutoff: 3100, q: 0.9, sampleRate: sampleRate); bowR.set(cutoff: 3300, q: 0.9, sampleRate: sampleRate)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let sr = sampleRate
        var dts = [Float](repeating: 0, count: oscs.count)
        let gain = 0.2 / sqrtf(Float(oscs.count)) * (0.45 + 0.55 * velocity)
        var i = 0
        while i < count {
            let block = min(16, count - i)
            let t = Float(elapsed) / sr
            let abs = control.time + Float(i) / sr
            let vib = vibrato * smoothstep((t - 0.25) / 0.5) * sinf(twoPi * 5.2 * t)
            let base = midiToHz(pitch() + vib)
            for k in 0..<oscs.count { dts[k] = base * powf(2, detunes[k] + 0.03 / 12 * sinf(twoPi * drifts[k] * abs + Float(k))) / sr }
            let open = spiccato ? expf(-t / 0.08) : smoothstep(t / max(0.05, attack * 1.6))
            let fc = (500 + brightness * 600) * powf(2, (1.4 + 1.8 * brightness) * open) * control.cutoff * (0.75 + 0.25 * velocity)
            filterL.set(cutoff: fc, resonance: 0.08, sampleRate: sr)
            filterR.set(cutoff: fc * 1.02, resonance: 0.08, sampleRate: sr)
            for j in 0..<block {
                if elapsed == gateSamples { amp.gateOff() }
                var sl: Float = 0, srr: Float = 0
                for k in 0..<oscs.count {
                    let s = oscs[k].saw(dts[k])
                    sl += s * pans[k].0; srr += s * pans[k].1
                }
                let a = amp.next()
                var yl = filterL.process(sl, drive: 0.1), yr = filterR.process(srr, drive: 0.1)
                yl += bodyL.bandpass(yl) * 0.5; yr += bodyR.bandpass(yr) * 0.5
                // Rosin: a little bow noise, strongest on the attack.
                let bow = (spiccato ? 0.09 : 0.025) * (0.4 + 0.6 * expf(-Float(elapsed) / (0.05 * sr)))
                yl += bowL.bandpass(rng.bipolar()) * bow; yr += bowR.bandpass(rng.bipolar()) * bow
                l[i + j] += yl * a * gain * 1.414
                r[i + j] += yr * a * gain * 1.414
                elapsed += 1
            }
            i += block
        }
        return !amp.finished
    }
}

// MARK: - brass and braam

/// Brass section: detuned saws, a lip scoop and a filter that blats open.
/// ``params.braam`` turns it into the trailer low-brass cluster (root,
/// fifth and octave, over-driven, a slow massive opening and a sub).
final class BrassVoice: Voice {
    var oscs: [Osc] = []
    var ratios: [Float] = []
    var filterL = Ladder(), filterR = Ladder()
    var sub = Osc()
    var breath = SVF()
    var amp: ADSR
    let braam: Bool, drive: Float, brightness: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        braam = p.f("braam", 0) > 0.5
        drive = p.f("drive", braam ? 1.4 : 0.35)
        brightness = p.f("brightness", 0.6)
        var r = Rng(seed: hash64(seed, UInt64(event.index), 0xB2A))
        let stack: [Float] = braam ? [0, 0, 7, 12, 12, -12] : [0, 0, 0, 12]
        for (k, interval) in stack.enumerated() {
            ratios.append(powf(2, (interval + (Float(k % 2) * 2 - 1) * (braam ? 0.11 : 0.07)) / 12))
            oscs.append(Osc(phase: r.unit()))
        }
        amp = ADSR(attack: p.f("attack", braam ? 0.02 : 0.025), decay: braam ? 2.5 : 0.4, sustain: braam ? 0.75 : 0.78,
                   release: p.f("release", braam ? 1.6 : 0.18), sampleRate: sampleRate)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        breath.set(cutoff: braam ? 420 : 1600, q: 0.8, sampleRate: sampleRate)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let sr = sampleRate
        let gain = (braam ? 0.11 : 0.15) / sqrtf(Float(oscs.count)) * (0.4 + 0.6 * velocity)
        var i = 0
        while i < count {
            let block = min(16, count - i)
            let t = Float(elapsed) / sr
            // Lips scoop into the note; the braam leans in from below.
            let scoop = (braam ? -0.6 : -0.35) * expf(-t / (braam ? 0.08 : 0.04))
            let f = midiToHz(pitch() + scoop)
            let blat: Float = braam
                ? (t < 0.55 ? powf(t / 0.55, 0.7) : 0.55 + 0.45 * expf(-(t - 0.55) / 1.6))
                : min(1, t / 0.03) * (0.45 + 0.55 * expf(-t / 0.3))
            let fc = (braam ? 110 : 420) * powf(2, (braam ? 4.4 : 3.2) * blat * (0.5 + 0.5 * brightness) * (0.6 + 0.4 * velocity)) * control.cutoff
            filterL.set(cutoff: fc, resonance: braam ? 0.28 : 0.15, sampleRate: sr)
            filterR.set(cutoff: fc * 1.015, resonance: braam ? 0.28 : 0.15, sampleRate: sr)
            for j in 0..<block {
                if elapsed == gateSamples { amp.gateOff() }
                var sl: Float = 0, srr: Float = 0
                for k in 0..<oscs.count {
                    let s = oscs[k].saw(f * ratios[k] / sr)
                    if k % 2 == 0 { sl += s; srr += s * 0.6 } else { srr += s; sl += s * 0.6 }
                }
                let a = amp.next()
                var yl = filterL.process(sl, drive: drive), yr = filterR.process(srr, drive: drive)
                if braam {
                    yl = saturate(yl * 1.6, 0.8); yr = saturate(yr * 1.6, 0.8)
                    let s = sub.sine(f * 0.5 / sr) * 0.55
                    yl += s; yr += s
                }
                let n = breath.bandpass(rng.bipolar()) * 0.04 * expf(-t / 0.2)
                l[i + j] += (yl + n) * a * gain * panL * 1.414
                r[i + j] += (yr + n) * a * gain * panR * 1.414
                elapsed += 1
            }
            i += block
        }
        return !amp.finished
    }
}

// MARK: - growl / wobble bass

/// Talking bass: FM through a wavefolder and a moving vowel band, synced to
/// the tempo (``params.lfo_hz``), over a clean mono sub. ``wobble`` swaps the
/// FM for a resonant low-pass sweep over detuned saws.
final class GrowlVoice: Voice {
    var car = Osc(), car2 = Osc(), mod = Osc(), sub = Osc(), sawA = Osc(), sawB = Osc(), sawC = Osc()
    var vowelL = SVF(), vowelR = SVF(), vowel2L = SVF(), vowel2R = SVF(), hpL = SVF(), hpR = SVF(), hp2L = SVF(), hp2R = SVF(), subLP = SVF()
    var ladder = Ladder()
    var amp: ADSR
    let lfoHz: Float, ratio: Float, index: Float, fold: Float, wobble: Bool, subLevel: Float, lfoPhase: Float

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        let p = spec.params
        lfoHz = p.f("lfo_hz", 3)
        ratio = p.f("ratio", 1)
        index = p.f("index", 3.2)
        fold = p.f("fold", 2.2)
        wobble = p.f("wobble", 0) > 0.5
        subLevel = p.f("sub", 0.8)
        // The event's variant shifts the LFO phase so riffs talk differently.
        lfoPhase = p.f("lfo_phase", 0)
        amp = ADSR(attack: 0.004, decay: 0.3, sustain: 1, release: p.f("release", 0.07), sampleRate: sampleRate)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        hpL.set(cutoff: 95, q: 0.7, sampleRate: sampleRate); hpR.set(cutoff: 95, q: 0.7, sampleRate: sampleRate)
        hp2L.set(cutoff: 95, q: 0.7, sampleRate: sampleRate); hp2R.set(cutoff: 95, q: 0.7, sampleRate: sampleRate)
        subLP.set(cutoff: 110, q: 0.7, sampleRate: sampleRate)
        sawB.phase = 0.33; sawC.phase = 0.66
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let sr = sampleRate
        var i = 0
        while i < count {
            let block = min(8, count - i)
            let t = Float(elapsed) / sr
            let lfo = 0.5 - 0.5 * cosf(twoPi * (lfoHz * t + lfoPhase))
            let f = midiToHz(pitch())
            if wobble {
                ladder.set(cutoff: 90 * powf(2, 5.6 * lfo) * control.cutoff, resonance: 0.55, sampleRate: sr)
            } else {
                // Two formants moving against each other: "ow" → "ee".
                let center = 380 + 900 * lfo, second = 2600 - 1500 * lfo
                vowelL.set(cutoff: center, q: 3.5, sampleRate: sr)
                vowelR.set(cutoff: center * 1.08, q: 3.5, sampleRate: sr)
                vowel2L.set(cutoff: second, q: 4.5, sampleRate: sr)
                vowel2R.set(cutoff: second * 0.95, q: 4.5, sampleRate: sr)
            }
            for j in 0..<block {
                if elapsed == gateSamples { amp.gateOff() }
                var gl: Float, gr: Float
                if wobble {
                    let s = 0.4 * sawA.saw(f / sr) + 0.35 * sawB.saw(f * 1.006 / sr) + 0.35 * sawC.saw(f * 0.994 / sr)
                    let y = saturate(ladder.process(s, drive: 0.6) * 1.5, 0.7)
                    gl = y; gr = y
                } else {
                    let idx = index * (0.25 + 0.75 * lfo)
                    let m = mod.sine(f * ratio / sr) * idx
                    let cl = sinf(twoPi * car.phase + m), cr = sinf(twoPi * car2.phase + m)
                    car.advance(f / sr); car2.advance(f * 1.004 / sr)
                    let k = fold * (1.2 + 1.8 * lfo)
                    let fl = sinf(cl * k), fr = sinf(cr * k)
                    gl = fl * 0.35 + vowelL.bandpass(fl) * 1.7 + vowel2L.bandpass(fl) * 1.3
                    gr = fr * 0.35 + vowelR.bandpass(fr) * 1.7 + vowel2R.bandpass(fr) * 1.3
                    // Drive after the vowel: the screech lives above 1 kHz.
                    gl = saturate(gl * 1.8, 1.2) * 1.6; gr = saturate(gr * 1.8, 1.2) * 1.6
                }
                gl = hp2L.highpass(hpL.highpass(gl)); gr = hp2R.highpass(hpR.highpass(gr))
                let s = subLP.lowpass(sub.sine(f / sr)) * subLevel * 1.2
                let a = amp.next() * 0.3 * (0.5 + 0.5 * velocity)
                l[i + j] += (gl * 1.2 + s) * a * 1.414 * panL
                r[i + j] += (gr * 1.2 + s) * a * 1.414 * panR
                elapsed += 1
            }
            i += block
        }
        return !amp.finished
    }
}

// MARK: - log drum

/// Amapiano log drum: a pitched, punchy body that drops into its note and
/// rings round and woody, with a little grit.
final class LogDrumVoice: Voice {
    var osc = Osc(), osc2 = Osc()
    var lp = SVF()
    let decay: Float
    var level: Float = 1

    override init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64) {
        decay = spec.params.f("decay", 1) * (event.duration > 0.4 ? 1.3 : 0.9)
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        lp.set(cutoff: 1100, q: 0.9, sampleRate: sampleRate)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let sr = sampleRate
        for i in 0..<count {
            let t = Float(elapsed) / sr
            let f = midiToHz(pitch() + 11 * expf(-t / 0.018))
            let body = osc.sine(f / sr) + 0.35 * osc2.triangle(f * 2.01 / sr) * expf(-t / 0.06)
            let env = min(1, t / 0.0012) * expf(-t / (0.2 * decay)) * (elapsed < gateSamples ? 1 : expf(-Float(elapsed - gateSamples) / (0.04 * sr)))
            level = env
            let s = lp.lowpass(warm(body * 1.3, 0.8)) * env * 0.42 * (0.4 + 0.6 * velocity)
            l[i] += s * panL * 1.414
            r[i] += s * panR * 1.414
            elapsed += 1
        }
        return level > 1e-4 || elapsed < 200
    }
}
