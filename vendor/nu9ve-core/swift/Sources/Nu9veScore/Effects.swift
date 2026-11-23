import Foundation

/// Eight-line feedback delay network reverb (Householder mixing, damped
/// lines, gently modulated): dense and smooth, cheap enough for long renders.
final class FDNReverb {
    let lines: [DelayLine]
    var lengths: [Float]
    let baseLengths: [Float]
    var gains: [Float]
    var damping: [OnePole]
    let diffusersL: [Allpass], diffusersR: [Allpass]
    let predelay: DelayLine
    let predelaySamples: Float
    var inputHP = SVF(), inputHPR = SVF()
    var toneLP = SVF(), toneLPR = SVF()
    // Returns lose their lows: reverb below ~170 Hz only smears the bass.
    var outHP = SVF(), outHPR = SVF()
    let sampleRate: Float
    var modPhase: Float = 0
    let modRate: Float
    let modDepth: Float
    var outputGain: Float

    /// Revision 2 cuts the returns below 170 Hz; revision 1 returns them whole.
    let lowCut: Bool

    init(rt60: Float, size: Float, damp: Float, predelay seconds: Float, sampleRate: Float, gainDb: Float = 0, lowCut: Bool = true) {
        self.lowCut = lowCut
        self.sampleRate = sampleRate
        let baseMs: [Float] = [29.7, 37.1, 41.1, 43.7, 53.3, 59.9, 67.7, 73.1]
        baseLengths = baseMs.map { $0 * 0.001 * sampleRate * max(0.2, size) }
        lengths = baseLengths
        lines = baseLengths.map { DelayLine(maxSamples: Int($0) + 64) }
        gains = baseLengths.map { powf(10, -3 * $0 / (max(0.1, rt60) * sampleRate)) }
        damping = baseLengths.map { _ in
            var p = OnePole()
            p.setLowpass(cutoff: damp, sampleRate: sampleRate)
            return p
        }
        let diffusion: [Float] = [4.7, 3.6, 12.7, 9.3]
        diffusersL = diffusion.map { Allpass(samples: Int($0 * 0.001 * sampleRate), gain: 0.62) }
        diffusersR = diffusion.map { Allpass(samples: Int($0 * 0.001 * sampleRate * 1.07), gain: 0.62) }
        predelaySamples = max(1, seconds * sampleRate)
        predelay = DelayLine(maxSamples: Int(predelaySamples) + 4)
        inputHP.set(cutoff: 180, q: 0.7, sampleRate: sampleRate)
        inputHPR.set(cutoff: 180, q: 0.7, sampleRate: sampleRate)
        toneLP.set(cutoff: min(12000, damp * 1.6), q: 0.7, sampleRate: sampleRate)
        toneLPR.set(cutoff: min(12000, damp * 1.6), q: 0.7, sampleRate: sampleRate)
        outHP.set(cutoff: 170, q: 0.6, sampleRate: sampleRate)
        outHPR.set(cutoff: 170, q: 0.6, sampleRate: sampleRate)
        modRate = 0.27
        modDepth = 6 * max(0.3, size)
        outputGain = dbToGain(gainDb) * 0.55
    }

    /// Mono-ish input pair in, stereo wet out (added to ``outL/outR``).
    /// ``sample`` is the absolute position of the first input sample: the
    /// modulation follows it so a window renders like the full piece.
    func process(inL: UnsafePointer<Float>, inR: UnsafePointer<Float>, outL: UnsafeMutablePointer<Float>,
                 outR: UnsafeMutablePointer<Float>, count: Int, sample: Int) {
        var taps = [Float](repeating: 0, count: 8)
        let cycle = Double(modRate) / Double(sampleRate) * Double(sample)
        modPhase = Float(cycle - floor(cycle))
        for i in 0..<count {
            let mono = (inL[i] + inR[i]) * 0.5
            predelay.push(mono)
            var x = predelay.read(predelaySamples)
            var xl = inputHP.highpass(x + (inL[i] - inR[i]) * 0.25)
            var xr = inputHPR.highpass(x - (inL[i] - inR[i]) * 0.25)
            for d in diffusersL { xl = d.process(xl) }
            for d in diffusersR { xr = d.process(xr) }
            modPhase += modRate / sampleRate
            if modPhase >= 1 { modPhase -= 1 }
            let mod = sinf(twoPi * modPhase) * modDepth
            var sum: Float = 0
            for k in 0..<8 {
                let length = baseLengths[k] + (k == 1 ? mod : k == 6 ? -mod : 0)
                let value = damping[k].lowpass(lines[k].read(length))
                taps[k] = value
                sum += value
            }
            let h = sum * 0.25
            var l: Float = 0, r: Float = 0
            for k in 0..<8 {
                let mixed = (taps[k] - h) * gains[k]
                x = (k % 2 == 0 ? xl : xr)
                lines[k].push(x + mixed)
                if k % 2 == 0 { l += taps[k] * (k % 4 == 0 ? 1 : -1) } else { r += taps[k] * (k % 4 == 1 ? 1 : -1) }
            }
            outL[i] += (lowCut ? outHP.highpass(toneLP.lowpass(l)) : toneLP.lowpass(l)) * outputGain
            outR[i] += (lowCut ? outHPR.highpass(toneLPR.lowpass(r)) : toneLPR.lowpass(r)) * outputGain
        }
    }
}

/// Ping-pong delay with filtered, softly saturated feedback. Two units
/// alternate between tempo segments so a tail keeps its own timing while the
/// next section already repeats at the new tempo.
final class PingPongDelay {
    let left: DelayLine, right: DelayLine
    var timeL: Float, timeR: Float, feedback: Float
    var hpL = SVF(), hpR = SVF(), lpL = SVF(), lpR = SVF()
    let sampleRate: Float
    var smoothTimeL: Float, smoothTimeR: Float

    init(sampleRate: Float, maxSeconds: Float = 2.5) {
        self.sampleRate = sampleRate
        left = DelayLine(maxSamples: Int(maxSeconds * sampleRate))
        right = DelayLine(maxSamples: Int(maxSeconds * sampleRate))
        timeL = 0.375 * sampleRate
        timeR = 0.5 * sampleRate
        smoothTimeL = timeL
        smoothTimeR = timeR
        feedback = 0.35
        configure(tone: 3800)
    }

    func configure(tone: Float) {
        hpL.set(cutoff: 260, q: 0.7, sampleRate: sampleRate)
        hpR.set(cutoff: 260, q: 0.7, sampleRate: sampleRate)
        lpL.set(cutoff: tone, q: 0.7, sampleRate: sampleRate)
        lpR.set(cutoff: tone, q: 0.7, sampleRate: sampleRate)
    }

    func process(inL: UnsafePointer<Float>?, inR: UnsafePointer<Float>?, outL: UnsafeMutablePointer<Float>,
                 outR: UnsafeMutablePointer<Float>, count: Int) {
        for i in 0..<count {
            smoothTimeL += (timeL - smoothTimeL) * 0.0005
            smoothTimeR += (timeR - smoothTimeR) * 0.0005
            let dl = left.read(smoothTimeL)
            let dr = right.read(smoothTimeR)
            let input = inL.map { ($0[i] + inR![i]) * 0.5 } ?? 0
            // Ping-pong: the left repeat feeds the right line and vice versa.
            let fbL = lpL.lowpass(hpL.highpass(dr)) * feedback
            let fbR = lpR.lowpass(hpR.highpass(dl)) * feedback
            left.push(saturate(input + fbL, 0.1))
            right.push(saturate(fbR, 0.1))
            outL[i] += dl
            outR[i] += dr
        }
    }
}

/// Stereo chorus (two modulated short delays).
final class Chorus {
    let left: DelayLine, right: DelayLine
    let sampleRate: Float
    let depth: Float
    init(sampleRate: Float) {
        self.sampleRate = sampleRate
        left = DelayLine(maxSamples: Int(0.04 * sampleRate))
        right = DelayLine(maxSamples: Int(0.04 * sampleRate))
        depth = 0.0028 * sampleRate
    }
    func process(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, count: Int, mix: Float, time: Float) {
        for i in 0..<count {
            let t = time + Float(i) / sampleRate
            left.push(l[i])
            right.push(r[i])
            let ml = left.read(0.012 * sampleRate + depth * (1 + sinf(twoPi * 0.31 * t)))
            let mr = right.read(0.0135 * sampleRate + depth * (1 + sinf(twoPi * 0.37 * t + 1.7)))
            l[i] = l[i] * (1 - mix * 0.5) + mr * mix * 0.7
            r[i] = r[i] * (1 - mix * 0.5) + ml * mix * 0.7
        }
    }
}

/// Bit depth and sample-rate reduction (chip, industrial, hacker textures).
struct Crusher {
    var holdL: Float = 0, holdR: Float = 0
    var counter: Float = 0
    mutating func process(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, count: Int,
                          bits: Float, rateDivider: Float) {
        let levels = bits > 0 ? powf(2, bits - 1) : 0
        for i in 0..<count {
            counter += 1
            if counter >= max(1, rateDivider) {
                counter = 0
                holdL = l[i]
                holdR = r[i]
                if levels > 0 {
                    holdL = roundf(holdL * levels) / levels
                    holdR = roundf(holdR * levels) / levels
                }
            }
            l[i] = holdL
            r[i] = holdR
        }
    }
}

/// RMS glue compressor, stereo linked, soft knee.
struct GlueCompressor {
    var threshold: Float
    var ratio: Float
    var knee: Float = 6
    var attack: Float
    var release: Float
    var power: Float = 0
    var gainDb: Float = 0
    let powerCoef: Float
    var maxReduction: Float = 0

    init(thresholdDb: Float, ratio: Float, attack: Float, release: Float, sampleRate: Float) {
        threshold = thresholdDb
        self.ratio = ratio
        self.attack = expf(-1 / (attack * sampleRate))
        self.release = expf(-1 / (release * sampleRate))
        powerCoef = expf(-1 / (0.02 * sampleRate))
    }

    mutating func process(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, count: Int) {
        for i in 0..<count {
            let p = max(l[i] * l[i], r[i] * r[i])
            power = p + powerCoef * (power - p)
            let level = 10 * log10f(power + 1e-12)
            let over = level - threshold
            var target: Float = 0
            if over > knee / 2 {
                target = -over * (1 - 1 / ratio)
            } else if over > -knee / 2 {
                let x = over + knee / 2
                target = -(1 - 1 / ratio) * x * x / (2 * knee)
            }
            let coef = target < gainDb ? attack : release
            gainDb = target + coef * (gainDb - target)
            maxReduction = min(maxReduction, gainDb)
            let g = dbToGain(gainDb)
            l[i] *= g
            r[i] *= g
        }
    }
}

/// Look-ahead peak limiter working on 32-sample chunks: the gain reaches its
/// target before the peak leaves the delay; release is exponential.
final class Limiter {
    let ceiling: Float
    let chunk = 32
    let lookChunks = 4
    var delayL: [Float], delayR: [Float]
    var chunkPeaks: [Float]
    var gain: Float = 1
    let releaseCoef: Float
    var pendingL: [Float] = [], pendingR: [Float] = []
    var minGain: Float = 1

    init(ceilingDb: Float, sampleRate: Float) {
        ceiling = dbToGain(ceilingDb)
        delayL = []
        delayR = []
        chunkPeaks = []
        releaseCoef = expf(-Float(32) / (0.12 * sampleRate))
    }

    /// Feeds samples; returns limited samples delayed by the look-ahead.
    func process(_ l: UnsafePointer<Float>, _ r: UnsafePointer<Float>, count: Int,
                 outL: inout [Float], outR: inout [Float]) {
        pendingL.append(contentsOf: UnsafeBufferPointer(start: l, count: count))
        pendingR.append(contentsOf: UnsafeBufferPointer(start: r, count: count))
        while pendingL.count >= chunk {
            var peak: Float = 0
            for i in 0..<chunk { peak = max(peak, abs(pendingL[i]), abs(pendingR[i])) }
            delayL.append(contentsOf: pendingL[0..<chunk])
            delayR.append(contentsOf: pendingR[0..<chunk])
            pendingL.removeFirst(chunk)
            pendingR.removeFirst(chunk)
            chunkPeaks.append(peak)
            if chunkPeaks.count > lookChunks {
                emitChunk(outL: &outL, outR: &outR)
            }
        }
    }

    private func emitChunk(outL: inout [Float], outR: inout [Float]) {
        let upcoming = chunkPeaks.prefix(lookChunks + 1).max() ?? 0
        let target = upcoming > ceiling ? ceiling / upcoming : 1
        let start = gain
        let end = target < gain ? target : target + (gain - target) * releaseCoef
        for i in 0..<chunk {
            let g = start + (end - start) * Float(i + 1) / Float(chunk)
            var a = delayL[i] * g, b = delayR[i] * g
            // Final safety: soft clip anything the chunk ramp still lets through.
            if abs(a) > ceiling { a = ceiling * tanhf(a / ceiling) }
            if abs(b) > ceiling { b = ceiling * tanhf(b / ceiling) }
            outL.append(a)
            outR.append(b)
        }
        gain = end
        minGain = min(minGain, gain)
        delayL.removeFirst(chunk)
        delayR.removeFirst(chunk)
        chunkPeaks.removeFirst()
    }

    func flush(outL: inout [Float], outR: inout [Float]) {
        let silence = [Float](repeating: 0, count: chunk * (lookChunks + 1))
        silence.withUnsafeBufferPointer { zero in
            process(zero.baseAddress!, zero.baseAddress!, count: zero.count, outL: &outL, outR: &outR)
        }
    }
}

/// Approximate BS.1770 integrated loudness (K-weighting, 400 ms blocks,
/// absolute and relative gates) accumulated while streaming.
final class LoudnessMeter {
    var shelf = Biquad(), highpass = Biquad()
    var shelfR = Biquad(), highpassR = Biquad()
    var blockEnergy: Double = 0
    var blockCount = 0
    let blockSize: Int
    var blocks: [Double] = []
    var peak: Float = 0

    init(sampleRate: Float) {
        blockSize = Int(0.4 * sampleRate)
        shelf = Biquad.highShelf(frequency: 1681, gainDb: 4, q: 0.707, sampleRate: sampleRate)
        shelfR = shelf
        highpass = Biquad.highpass(frequency: 38, q: 0.5, sampleRate: sampleRate)
        highpassR = highpass
    }

    func process(_ l: [Float], _ r: [Float]) {
        for i in 0..<l.count {
            peak = max(peak, abs(l[i]), abs(r[i]))
            let a = highpass.process(shelf.process(l[i]))
            let b = highpassR.process(shelfR.process(r[i]))
            blockEnergy += Double(a * a + b * b)
            blockCount += 1
            if blockCount == blockSize {
                blocks.append(blockEnergy / Double(blockSize))
                blockEnergy = 0
                blockCount = 0
            }
        }
    }

    var integrated: Double {
        let loud = blocks.filter { -0.691 + 10 * log10($0 + 1e-12) > -70 }
        guard !loud.isEmpty else { return -70 }
        let mean = loud.reduce(0, +) / Double(loud.count)
        let relative = -0.691 + 10 * log10(mean + 1e-12) - 10
        let gated = loud.filter { -0.691 + 10 * log10($0 + 1e-12) > relative }
        guard !gated.isEmpty else { return -70 }
        return -0.691 + 10 * log10(gated.reduce(0, +) / Double(gated.count) + 1e-12)
    }
}

struct Biquad {
    var b0: Float = 1, b1: Float = 0, b2: Float = 0, a1: Float = 0, a2: Float = 0
    var z1: Float = 0, z2: Float = 0

    @inline(__always) mutating func process(_ x: Float) -> Float {
        let y = b0 * x + z1
        z1 = b1 * x - a1 * y + z2
        z2 = b2 * x - a2 * y
        return y
    }

    static func highShelf(frequency: Float, gainDb: Float, q: Float, sampleRate: Float) -> Biquad {
        let a = powf(10, gainDb / 40)
        let w = twoPi * frequency / sampleRate
        let alpha = sinf(w) / (2 * q)
        let c = cosf(w)
        let sa = 2 * sqrtf(a) * alpha
        let a0 = (a + 1) - (a - 1) * c + sa
        var f = Biquad()
        f.b0 = a * ((a + 1) + (a - 1) * c + sa) / a0
        f.b1 = -2 * a * ((a - 1) + (a + 1) * c) / a0
        f.b2 = a * ((a + 1) + (a - 1) * c - sa) / a0
        f.a1 = 2 * ((a - 1) - (a + 1) * c) / a0
        f.a2 = ((a + 1) - (a - 1) * c - sa) / a0
        return f
    }

    static func lowShelf(frequency: Float, gainDb: Float, q: Float, sampleRate: Float) -> Biquad {
        let a = powf(10, gainDb / 40)
        let w = twoPi * frequency / sampleRate
        let alpha = sinf(w) / (2 * q)
        let c = cosf(w)
        let sa = 2 * sqrtf(a) * alpha
        let a0 = (a + 1) + (a - 1) * c + sa
        var f = Biquad()
        f.b0 = a * ((a + 1) - (a - 1) * c + sa) / a0
        f.b1 = 2 * a * ((a - 1) - (a + 1) * c) / a0
        f.b2 = a * ((a + 1) - (a - 1) * c - sa) / a0
        f.a1 = -2 * ((a - 1) + (a + 1) * c) / a0
        f.a2 = ((a + 1) + (a - 1) * c - sa) / a0
        return f
    }

    static func highpass(frequency: Float, q: Float, sampleRate: Float) -> Biquad {
        let w = twoPi * frequency / sampleRate
        let alpha = sinf(w) / (2 * q)
        let c = cosf(w)
        let a0 = 1 + alpha
        var f = Biquad()
        f.b0 = (1 + c) / 2 / a0
        f.b1 = -(1 + c) / a0
        f.b2 = (1 + c) / 2 / a0
        f.a1 = -2 * c / a0
        f.a2 = (1 - alpha) / a0
        return f
    }
}
