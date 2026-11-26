import Foundation

// Shared DSP primitives for the score renderer. Everything is Float, sample
// by sample, allocation-free inside render loops and deterministic: noise
// comes from seeded xorshift generators, LFOs read absolute time.

let twoPi: Float = 2 * .pi

@inline(__always) func clamp01(_ x: Float) -> Float { min(1, max(0, x)) }
@inline(__always) func dbToGain(_ db: Float) -> Float { powf(10, db / 20) }
@inline(__always) func midiToHz(_ note: Float) -> Float { 440 * powf(2, (note - 69) / 12) }
@inline(__always) func lerp(_ a: Float, _ b: Float, _ t: Float) -> Float { a + (b - a) * t }
@inline(__always) func smoothstep(_ x: Float) -> Float { let t = clamp01(x); return t * t * (3 - 2 * t) }

/// Soft saturation with exactly unity slope at zero (safe inside feedback
/// loops): small signals pass untouched, peaks round off towards 1/k.
@inline(__always) func saturate(_ x: Float, _ drive: Float) -> Float {
    if drive <= 0.001 { return x }
    let k = 1 + drive * 6
    return tanhf(x * k) / k
}

/// Bipolar LFO value for a phase in cycles: sine, triangle, square or
/// sample-and-hold (a seeded step per cycle, the same on every render).
@inline(__always) func lfoValue(_ phase: Float, _ shape: String, seed: UInt64) -> Float {
    switch shape {
    case "tri": let x = phase - floorf(phase); return 4 * abs(x - 0.5) - 1
    case "square": return phase - floorf(phase) < 0.5 ? 1 : -1
    case "sh":
        let step = UInt64(Int64(floorf(phase)) & 0xffffffff)
        return Float(hash64(seed, step) >> 40) / Float(1 << 24) * 2 - 1
    default: return sinf(twoPi * phase)
    }
}

/// Asymmetric tube-ish saturation (even harmonics for bass and keys), unity
/// slope at zero.
@inline(__always) func warm(_ x: Float, _ drive: Float) -> Float {
    if drive <= 0.001 { return x }
    let k = 1 + drive * 4
    let y = x * k
    return (y >= 0 ? tanhf(y) : tanhf(y * 0.82) / 0.82) / k
}

/// Slow analog pitch drift in semitones for oscillator `k`: two incommensurate
/// sines per voice, so unison partners wander apart and back (0 when cents is 0).
@inline(__always) func analogDrift(_ t: Float, _ k: Int, cents: Float, phase: Float) -> Float {
    if cents <= 0 { return 0 }
    let a = sinf(twoPi * ((0.071 + 0.037 * Float(k)) * t + phase))
    let b = sinf(twoPi * ((0.23 + 0.051 * Float(k)) * t + phase * 1.7))
    return (0.7 * a + 0.3 * b) * cents / 100
}

struct Rng {
    var state: UInt32
    init(seed: UInt64) {
        var s = UInt32(truncatingIfNeeded: seed ^ (seed >> 32)) &* 2654435761
        if s == 0 { s = 0x9E3779B9 }
        state = s
    }
    @inline(__always) mutating func next() -> UInt32 {
        state ^= state << 13
        state ^= state >> 17
        state ^= state << 5
        return state
    }
    /// Uniform in [0, 1).
    @inline(__always) mutating func unit() -> Float { Float(next() >> 8) * (1.0 / 16_777_216.0) }
    /// Uniform in [-1, 1).
    @inline(__always) mutating func bipolar() -> Float { unit() * 2 - 1 }
}

func hash64(_ values: UInt64...) -> UInt64 {
    var h: UInt64 = 0xcbf29ce484222325
    for value in values {
        var v = value
        for _ in 0..<8 {
            h ^= v & 0xff
            h = h &* 0x100000001b3
            v >>= 8
        }
    }
    return h
}

/// Pink-ish noise (Paul Kellet's economy filter): warmer than white.
struct PinkNoise {
    var b0: Float = 0, b1: Float = 0, b2: Float = 0
    @inline(__always) mutating func process(_ white: Float) -> Float {
        b0 = 0.99765 * b0 + white * 0.0990460
        b1 = 0.96300 * b1 + white * 0.2965164
        b2 = 0.57000 * b2 + white * 1.0526913
        return (b0 + b1 + b2 + white * 0.1848) * 0.2
    }
}

@inline(__always) func polyBlep(_ t: Float, _ dt: Float) -> Float {
    if t < dt {
        let x = t / dt
        return x + x - x * x - 1
    } else if t > 1 - dt {
        let x = (t - 1) / dt
        return x * x + x + x + 1
    }
    return 0
}

/// Band-limited oscillator (PolyBLEP saw/square/pulse, naive triangle/sine).
struct Osc {
    var phase: Float = 0
    @inline(__always) mutating func saw(_ dt: Float) -> Float {
        let value = 2 * phase - 1 - polyBlep(phase, dt)
        advance(dt)
        return value
    }
    @inline(__always) mutating func pulse(_ dt: Float, width: Float) -> Float {
        let w = min(0.95, max(0.05, width))
        var value: Float = phase < w ? 1 : -1
        value += polyBlep(phase, dt)
        var shifted = phase - w
        if shifted < 0 { shifted += 1 }
        value -= polyBlep(shifted, dt)
        advance(dt)
        return value
    }
    @inline(__always) mutating func triangle(_ dt: Float) -> Float {
        let value = 4 * abs(phase - 0.5) - 1
        advance(dt)
        return -value
    }
    @inline(__always) mutating func sine(_ dt: Float) -> Float {
        let value = sinf(twoPi * phase)
        advance(dt)
        return value
    }
    /// Raw square with no anti-aliasing: deliberate for chip voices.
    @inline(__always) mutating func chipPulse(_ dt: Float, width: Float) -> Float {
        let value: Float = phase < width ? 1 : -1
        advance(dt)
        return value
    }
    /// 4-bit stepped triangle like the NES triangle channel.
    @inline(__always) mutating func chipTriangle(_ dt: Float) -> Float {
        let tri = 1 - 4 * abs(phase - 0.5)
        advance(dt)
        return (floorf((tri * 0.5 + 0.5) * 15) / 15) * 2 - 1
    }
    /// Wavetable-style morph through sine → triangle → saw → square (position
    /// 0…3), each corner band-limited like its own oscillator. One phase drives
    /// them all, so a moving position sweeps the timbre without clicks.
    @inline(__always) mutating func morph(_ dt: Float, position: Float) -> Float {
        let p = min(3, max(0, position))
        let value: Float
        if p < 1 {
            let sine = sinf(twoPi * phase), tri = 1 - 4 * abs(phase - 0.5)
            value = sine + (tri - sine) * p
        } else if p < 2 {
            let tri = 1 - 4 * abs(phase - 0.5), saw = 2 * phase - 1 - polyBlep(phase, dt)
            value = tri + (saw - tri) * (p - 1)
        } else {
            let saw = 2 * phase - 1 - polyBlep(phase, dt)
            var square: Float = phase < 0.5 ? 1 : -1
            square += polyBlep(phase, dt)
            var shifted = phase - 0.5
            if shifted < 0 { shifted += 1 }
            square -= polyBlep(shifted, dt)
            value = saw + (square * 0.8 - saw) * (p - 2)
        }
        advance(dt)
        return value
    }
    /// One sample of a named wave (layers and unison voices).
    @inline(__always) mutating func wave(_ name: String, _ dt: Float) -> Float {
        switch name {
        case "sine": return sine(dt)
        case "tri": return triangle(dt)
        case "square": return pulse(dt, width: 0.5)
        default: return saw(dt)
        }
    }
    @inline(__always) mutating func advance(_ dt: Float) {
        phase += dt
        if phase >= 1 { phase -= floorf(phase) }
    }
}

/// Zero-delay-feedback state variable filter (Simper). Coefficients are
/// updated by the caller at control rate.
struct SVF {
    var ic1: Float = 0, ic2: Float = 0
    var a1: Float = 1, a2: Float = 0, a3: Float = 0, k: Float = 1.4

    @inline(__always) mutating func set(cutoff: Float, q: Float, sampleRate: Float) {
        let fc = min(max(cutoff, 10), sampleRate * 0.45)
        let g = tanf(.pi * fc / sampleRate)
        k = 1 / max(q, 0.05)
        a1 = 1 / (1 + g * (g + k))
        a2 = g * a1
        a3 = g * a2
    }
    /// Resonance 0..1 mapped to Q 0.5..~20.
    @inline(__always) mutating func set(cutoff: Float, resonance: Float, sampleRate: Float) {
        let r = clamp01(resonance)
        set(cutoff: cutoff, q: 0.5 + r * r * 19.5, sampleRate: sampleRate)
    }
    @inline(__always) mutating func tick(_ v0: Float) -> (lp: Float, bp: Float, hp: Float) {
        let v3 = v0 - ic2
        let v1 = a1 * ic1 + a2 * v3
        let v2 = ic2 + a2 * ic1 + a3 * v3
        ic1 = 2 * v1 - ic1
        ic2 = 2 * v2 - ic2
        return (v2, v1, v0 - k * v1 - v2)
    }
    @inline(__always) mutating func lowpass(_ x: Float) -> Float { tick(x).lp }
    @inline(__always) mutating func bandpass(_ x: Float) -> Float { tick(x).bp }
    @inline(__always) mutating func highpass(_ x: Float) -> Float { tick(x).hp }
    mutating func reset() { ic1 = 0; ic2 = 0 }
}

/// 24 dB/oct low-pass made of two SVFs with drive between stages: the
/// resonant first stage gives the "analog" peak, the second the slope.
struct Ladder {
    var a = SVF(), b = SVF()
    @inline(__always) mutating func set(cutoff: Float, resonance: Float, sampleRate: Float) {
        a.set(cutoff: cutoff, resonance: resonance, sampleRate: sampleRate)
        b.set(cutoff: cutoff, q: 0.6, sampleRate: sampleRate)
    }
    @inline(__always) mutating func process(_ x: Float, drive: Float) -> Float {
        let first = a.lowpass(x)
        return b.lowpass(drive > 0 ? saturate(first, drive * 0.5) : first)
    }
}

struct OnePole {
    var z: Float = 0
    var coefficient: Float = 0
    mutating func setLowpass(cutoff: Float, sampleRate: Float) {
        coefficient = expf(-twoPi * min(cutoff, sampleRate * 0.49) / sampleRate)
    }
    @inline(__always) mutating func lowpass(_ x: Float) -> Float {
        z = x + coefficient * (z - x)
        return z
    }
    @inline(__always) mutating func highpass(_ x: Float) -> Float { x - lowpass(x) }
}

/// ADSR with linear attack and exponential decay/release, in samples.
struct ADSR {
    enum Stage { case attack, decay, sustain, release, done }
    var stage: Stage = .attack
    var value: Float = 0
    var attackStep: Float = 1
    var decayCoef: Float = 0
    var releaseCoef: Float = 0
    var sustain: Float = 1

    init(attack: Float, decay: Float, sustain: Float, release: Float, sampleRate: Float) {
        attackStep = 1 / max(1, attack * sampleRate)
        decayCoef = expf(-1 / max(1, decay * sampleRate / 4.6))
        releaseCoef = expf(-1 / max(1, release * sampleRate / 6.9))
        self.sustain = sustain
    }
    mutating func gateOff() { if stage != .done { stage = .release } }
    @inline(__always) mutating func next() -> Float {
        switch stage {
        case .attack:
            value += attackStep
            if value >= 1 { value = 1; stage = .decay }
        case .decay:
            value = sustain + (value - sustain) * decayCoef
            if abs(value - sustain) < 1e-4 { value = sustain; stage = .sustain }
        case .sustain:
            break
        case .release:
            value *= releaseCoef
            if value < 1e-5 { value = 0; stage = .done }
        case .done:
            value = 0
        }
        return value
    }
    var finished: Bool { stage == .done }
}

/// Damped complex phasor: an exactly exponential decaying sinusoid for the
/// price of a complex multiply (modal bells, glass, membranes).
struct Resonator {
    var re: Float = 0, im: Float = 0
    var cr: Float = 1, ci: Float = 0
    init(frequency: Float, decay: Float, amplitude: Float, phase: Float, sampleRate: Float) {
        let radius = expf(-1 / max(1, decay * sampleRate / 6.9))
        let omega = twoPi * min(frequency, sampleRate * 0.47) / sampleRate
        cr = radius * cosf(omega)
        ci = radius * sinf(omega)
        re = amplitude * cosf(phase)
        im = amplitude * sinf(phase)
    }
    @inline(__always) mutating func next() -> Float {
        let r = re * cr - im * ci
        im = re * ci + im * cr
        re = r
        return im
    }
    var energy: Float { re * re + im * im }
}

/// Fractional delay line with linear interpolation.
final class DelayLine {
    var buffer: [Float]
    var write = 0
    let mask: Int
    init(maxSamples: Int) {
        var size = 1
        while size < maxSamples + 2 { size <<= 1 }
        buffer = [Float](repeating: 0, count: size)
        mask = size - 1
    }
    @inline(__always) func push(_ x: Float) {
        buffer[write] = x
        write = (write + 1) & mask
    }
    /// Value written ``delay`` samples ago (delay >= 1).
    @inline(__always) func read(_ delay: Float) -> Float {
        let d = max(1, delay)
        let whole = Int(d)
        let frac = d - Float(whole)
        let i0 = (write - whole) & mask
        let i1 = (i0 - 1) & mask
        return buffer[i0] + (buffer[i1] - buffer[i0]) * frac
    }
    @inline(__always) func readInt(_ delay: Int) -> Float { buffer[(write - delay) & mask] }
}

/// Schroeder allpass used for diffusion.
final class Allpass {
    let line: DelayLine
    let length: Int
    var gain: Float
    init(samples: Int, gain: Float) {
        line = DelayLine(maxSamples: samples + 1)
        length = samples
        self.gain = gain
    }
    @inline(__always) func process(_ x: Float) -> Float {
        let delayed = line.readInt(length)
        let v = x + gain * delayed
        line.push(v)
        return delayed - gain * v
    }
}

/// Piecewise-linear automation lane (absolute seconds).
struct Lane {
    var times: [Float]
    var values: [Float]
    var cursor = 0
    init(times: [Float], values: [Float]) {
        self.times = times
        self.values = values
    }
    mutating func value(at t: Float) -> Float {
        guard !times.isEmpty else { return 1 }
        if t <= times[0] { cursor = 0; return values[0] }
        if t >= times[times.count - 1] { cursor = times.count - 1; return values[values.count - 1] }
        if cursor >= times.count - 1 || times[cursor] > t { cursor = 0 }
        while cursor < times.count - 2 && times[cursor + 1] <= t { cursor += 1 }
        let t0 = times[cursor], t1 = times[cursor + 1]
        let u = (t - t0) / max(1e-6, t1 - t0)
        return values[cursor] + (values[cursor + 1] - values[cursor]) * u
    }
}

/// Evenly sampled control lane (e.g. the voice-duck lane at 20 Hz).
struct RateLane {
    let rate: Float
    let values: [Float]
    @inline(__always) func value(at t: Float) -> Float {
        guard !values.isEmpty else { return 0 }
        let position = max(0, t * rate)
        let i = Int(position)
        if i >= values.count - 1 { return values[values.count - 1] }
        let u = position - Float(i)
        return values[i] + (values[i + 1] - values[i]) * u
    }
}
