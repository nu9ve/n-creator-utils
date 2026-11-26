import AVFoundation
import Synchronization
import Foundation

/// Plays a region of an audio file (library song segments) through the same
/// buses, ducking and stems as the synths. ``params.rate`` > 1 is varispeed
/// (pitch and tempo together, nightcore-style).
final class SampleVoice: Voice {
    let audio: DecodedAudio
    var position: Double
    let rate: Double
    let fadeIn: Int, fadeOut: Int
    let total: Int

    init(event: EventSpec, spec: InstrumentSpec, sampleRate: Float, seed: UInt64, audio: DecodedAudio) {
        self.audio = audio
        let p = spec.params
        rate = Double(p.f("rate", 1))
        position = Double(p.f("offset", 0)) * Double(sampleRate) + Double(event.glide) * Double(sampleRate)
        fadeIn = max(1, Int(p.f("fade_in", 0.02) * sampleRate))
        fadeOut = max(1, Int(p.f("fade_out", 0.4) * sampleRate))
        total = max(1, Int(event.duration * Double(sampleRate)))
        super.init(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
    }

    override func render(_ l: UnsafeMutablePointer<Float>, _ r: UnsafeMutablePointer<Float>, _ count: Int, _ control: InstrumentControl) -> Bool {
        let length = audio.left.count
        for i in 0..<count {
            if elapsed >= total { return false }
            let index = Int(position)
            guard index + 1 < length else { return false }
            let frac = Float(position - Double(index))
            let a = audio.left[index] + (audio.left[index + 1] - audio.left[index]) * frac
            let b = audio.right[index] + (audio.right[index + 1] - audio.right[index]) * frac
            let fade = min(1, Float(elapsed) / Float(fadeIn)) * min(1, Float(total - elapsed) / Float(fadeOut))
            let g = fade * velocity
            l[i] += a * g * panL * 1.414
            r[i] += b * g * panR * 1.414
            position += rate
            elapsed += 1
        }
        return elapsed < total
    }
}

struct DecodedAudio: Sendable {
    let left: [Float]
    let right: [Float]
}

/// Set by the converter's input block, which runs synchronously inside `convert`.
private final class EndOfInput: @unchecked Sendable {
    var reached = false
}

enum AudioDecoder {
    /// Decoded samples by path, shared by every render in the process.
    private static let cache = Mutex<[String: DecodedAudio]>([:])

    static func load(_ path: String, sampleRate: Float) throws -> DecodedAudio {
        if let cached = cache.withLock({ $0[path] }) { return cached }
        let file = try AVAudioFile(forReading: URL(fileURLWithPath: path))
        guard let target = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: Double(sampleRate), channels: 2, interleaved: false),
              let converter = AVAudioConverter(from: file.processingFormat, to: target) else {
            throw ScoreError(description: "cannot decode \(path)")
        }
        var left: [Float] = [], right: [Float] = []
        let inputCapacity: AVAudioFrameCount = 32768
        let end = EndOfInput()
        while true {
            let capacity = AVAudioFrameCount(Double(inputCapacity) * Double(sampleRate) / file.processingFormat.sampleRate) + 1024
            guard let output = AVAudioPCMBuffer(pcmFormat: target, frameCapacity: capacity) else { break }
            var error: NSError?
            let status = converter.convert(to: output, error: &error) { _, outStatus in
                if end.reached { outStatus.pointee = .endOfStream; return nil }
                guard let input = AVAudioPCMBuffer(pcmFormat: file.processingFormat, frameCapacity: inputCapacity) else {
                    outStatus.pointee = .endOfStream; return nil
                }
                do { try file.read(into: input) } catch { end.reached = true; outStatus.pointee = .endOfStream; return nil }
                if input.frameLength == 0 { end.reached = true; outStatus.pointee = .endOfStream; return nil }
                outStatus.pointee = .haveData
                return input
            }
            if let error { throw error }
            let frames = Int(output.frameLength)
            if frames > 0, let data = output.floatChannelData {
                left.append(contentsOf: UnsafeBufferPointer(start: data[0], count: frames))
                right.append(contentsOf: UnsafeBufferPointer(start: data[target.channelCount > 1 ? 1 : 0], count: frames))
            }
            if status == .endOfStream || status == .error || (frames == 0 && end.reached) { break }
        }
        let decoded = DecodedAudio(left: left, right: right)
        cache.withLock { $0[path] = decoded }
        return decoded
    }
}

/// Streaming float WAV writer (stereo, IEEE float 32).
final class WavWriter {
    let handle: FileHandle
    let url: URL
    var frames = 0
    let sampleRate: Int

    init(path: String, sampleRate: Int) throws {
        url = URL(fileURLWithPath: path)
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        FileManager.default.createFile(atPath: path, contents: nil)
        handle = try FileHandle(forWritingTo: url)
        self.sampleRate = sampleRate
        handle.write(header(dataBytes: 0))
    }

    private func header(dataBytes: Int) -> Data {
        var data = Data()
        func u32(_ v: UInt32) { var x = v.littleEndian; withUnsafeBytes(of: &x) { data.append(contentsOf: $0) } }
        func u16(_ v: UInt16) { var x = v.littleEndian; withUnsafeBytes(of: &x) { data.append(contentsOf: $0) } }
        data.append(contentsOf: Array("RIFF".utf8)); u32(UInt32(36 + dataBytes))
        data.append(contentsOf: Array("WAVE".utf8))
        data.append(contentsOf: Array("fmt ".utf8)); u32(16); u16(3); u16(2)
        u32(UInt32(sampleRate)); u32(UInt32(sampleRate * 8)); u16(8); u16(32)
        data.append(contentsOf: Array("data".utf8)); u32(UInt32(dataBytes))
        return data
    }

    func write(_ l: UnsafePointer<Float>, _ r: UnsafePointer<Float>, count: Int) {
        guard count > 0 else { return }
        var interleaved = [Float](repeating: 0, count: count * 2)
        for i in 0..<count {
            interleaved[2 * i] = l[i]
            interleaved[2 * i + 1] = r[i]
        }
        interleaved.withUnsafeBytes { handle.write(Data($0)) }
        frames += count
    }

    func close() {
        handle.seek(toFileOffset: 0)
        handle.write(header(dataBytes: frames * 8))
        try? handle.close()
    }
}

/// Per-instrument runtime: block buffers, inserts, lanes and sends.
final class InstrumentRuntime {
    let spec: InstrumentSpec
    let index: Int
    let bufL: UnsafeMutablePointer<Float>
    let bufR: UnsafeMutablePointer<Float>
    var chorus: Chorus?
    var crusher = Crusher()
    var carveL = SVF(), carveR = SVF()
    var insertHPL = SVF(), insertHPR = SVF(), cabL = SVF(), cabR = SVF(), cab2L = SVF(), cab2R = SVF()
    var scoopL = SVF(), scoopR = SVF(), presL = SVF(), presR = SVF()
    var cutoffLane: Lane?
    var gainLane: Lane?
    var morphLane: Lane?
    var sendLanes: [String: Lane] = [:]
    var lastActive = -1_000_000_000
    let stemIndex: Int

    init(spec: InstrumentSpec, index: Int, block: Int, sampleRate: Float) {
        self.spec = spec
        self.index = index
        bufL = .allocate(capacity: block)
        bufR = .allocate(capacity: block)
        if spec.chorus > 0 { chorus = Chorus(sampleRate: sampleRate) }
        carveL.set(cutoff: 2000, q: 0.8, sampleRate: sampleRate)
        carveR.set(cutoff: 2000, q: 0.8, sampleRate: sampleRate)
        if spec.insertHighpass > 0 {
            insertHPL.set(cutoff: spec.insertHighpass, q: 0.7, sampleRate: sampleRate)
            insertHPR.set(cutoff: spec.insertHighpass, q: 0.7, sampleRate: sampleRate)
        }
        if spec.insertLowpass > 0 {
            // Cabinet: a resonant 12 dB stage and a gentle second one.
            cabL.set(cutoff: spec.insertLowpass, q: 0.9, sampleRate: sampleRate)
            cabR.set(cutoff: spec.insertLowpass, q: 0.9, sampleRate: sampleRate)
            cab2L.set(cutoff: spec.insertLowpass * 1.4, q: 0.6, sampleRate: sampleRate)
            cab2R.set(cutoff: spec.insertLowpass * 1.4, q: 0.6, sampleRate: sampleRate)
        }
        scoopL.set(cutoff: 480, q: 0.9, sampleRate: sampleRate); scoopR.set(cutoff: 480, q: 0.9, sampleRate: sampleRate)
        presL.set(cutoff: 1900, q: 1.1, sampleRate: sampleRate); presR.set(cutoff: 1900, q: 1.1, sampleRate: sampleRate)
        stemIndex = ScoreManifest.stems.firstIndex(of: spec.stem) ?? 2
    }

    deinit {
        bufL.deallocate()
        bufR.deallocate()
    }
}

/// What a render did. `schema` is the app's: it names the report its tools already read.
public struct RenderReport: Encodable, Sendable {
    public var schema: String
    public var output: String
    public var start: Double
    public var end: Double
    public var seconds: Double
    public var renderSeconds: Double
    public var realtimeFactor: Double
    public var peakDb: Double
    public var integratedLufs: Double
    public var glueMaxReductionDb: Double
    public var limiterMinGainDb: Double
    public var voices: Int
    public var maxPolyphony: Int
    public var nonFinite: Int
    public var stems: [String]
}

public final class ScoreEngine {
    public let manifest: ScoreManifest
    /// The `schema` written in every report.
    public let reportSchema: String
    let sampleRate: Float
    let block = 256
    let preroll: Double = 12.0

    public init(manifest: ScoreManifest, reportSchema: String) {
        self.reportSchema = reportSchema
        self.manifest = manifest
        sampleRate = manifest.sampleRate
    }

    func makeVoice(_ event: EventSpec) throws -> Voice {
        let spec = manifest.instruments[event.instrument]
        let seed = manifest.seed
        switch spec.kind {
        case "pad", "stab": return PadVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "keys": return KeysVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "modal": return ModalVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "pluck": return PluckVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "bass": return BassVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "lead": return LeadVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "drum": return DrumVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "texture": return TextureVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "membrane": return MembraneVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "bird": return BirdVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "conch": return ConchVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "whistle": return WhistleVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "fm": return FMVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "guitar": return GuitarVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "piano": return PianoVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "strings": return StringsVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "brass": return BrassVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "growl": return GrowlVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "logdrum": return LogDrumVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "fx": return FxVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed)
        case "sample":
            let audio = try AudioDecoder.load(spec.params.s("file", ""), sampleRate: sampleRate)
            return SampleVoice(event: event, spec: spec, sampleRate: sampleRate, seed: seed, audio: audio)
        default: throw ScoreError(description: "unknown instrument kind \(spec.kind)")
        }
    }

    /// Longest time a note of this instrument may keep sounding after its gate.
    func tail(_ spec: InstrumentSpec) -> Double {
        switch spec.kind {
        case "pad", "stab", "texture": return Double(spec.params.f("release", 3)) + 2
        case "modal": return Double(spec.params.f("decay", 2)) * 2.5 + 1
        case "drum": return 4
        case "fx": return 3.5
        case "membrane": return 3
        case "piano": return 8
        case "brass": return Double(spec.params.f("release", 1.6)) + 1
        default: return 6
        }
    }

    /// Renders `from`…`to` seconds (to the end when nil) to a 48 kHz float WAV, and each stem to `stemsDirectory`.
    public func render(output: String, from: Double, to: Double?, stemsDirectory: String?, progress: ((Double, Double) -> Void)? = nil) throws -> RenderReport {
        let clock = Date()
        var lastProgress = clock
        progress?(0, from)
        let end = min(to ?? manifest.duration, manifest.duration)
        let start = max(0, min(from, end))
        let renderStart = max(0, start - preroll)
        let sr = Double(sampleRate)
        let firstSample = Int((renderStart * sr).rounded())
        let outputFirst = Int((start * sr).rounded())
        let lastSample = Int((end * sr).rounded())

        let runtimes = manifest.instruments.enumerated().map { InstrumentRuntime(spec: $0.element, index: $0.offset, block: block, sampleRate: sampleRate) }
        for automation in manifest.automation {
            let runtime = runtimes[automation.instrument]
            if automation.parameter == "cutoff" { runtime.cutoffLane = automation.lane }
            else if automation.parameter == "gain" { runtime.gainLane = automation.lane }
            else if automation.parameter == "morph" { runtime.morphLane = automation.lane }
            else if automation.parameter.hasPrefix("send:") { runtime.sendLanes[String(automation.parameter.dropFirst(5))] = automation.lane }
        }
        let pumpTimes = manifest.events.filter { manifest.instruments[$0.instrument].pumpSource }.map { Float($0.start) }

        // Buses: three reverbs and two alternating delays.
        let reverbNames = ["room", "hall", "space"]
        let reverbs = reverbNames.map { name -> FDNReverb in
            let bus = manifest.buses[name]!
            return FDNReverb(rt60: bus.rt60, size: bus.size, damp: bus.damp, predelay: bus.predelay, sampleRate: sampleRate, gainDb: bus.gainDb,
                             lowCut: manifest.revision != .v1)
        }
        let delays = [PingPongDelay(sampleRate: sampleRate), PingPongDelay(sampleRate: sampleRate)]
        var delaySegment = -1

        let stemCount = ScoreManifest.stems.count
        let stemL = (0..<stemCount).map { _ in UnsafeMutablePointer<Float>.allocate(capacity: block) }
        let stemR = (0..<stemCount).map { _ in UnsafeMutablePointer<Float>.allocate(capacity: block) }
        let sendL = (0..<5).map { _ in UnsafeMutablePointer<Float>.allocate(capacity: block) }
        let sendR = (0..<5).map { _ in UnsafeMutablePointer<Float>.allocate(capacity: block) }
        let mixL = UnsafeMutablePointer<Float>.allocate(capacity: block)
        let mixR = UnsafeMutablePointer<Float>.allocate(capacity: block)
        let scratchL = UnsafeMutablePointer<Float>.allocate(capacity: block)
        let scratchR = UnsafeMutablePointer<Float>.allocate(capacity: block)
        defer {
            (stemL + stemR + sendL + sendR).forEach { $0.deallocate() }
            [mixL, mixR, scratchL, scratchR].forEach { $0.deallocate() }
        }

        let writer = try WavWriter(path: output, sampleRate: Int(sampleRate))
        var stemWriters: [WavWriter?] = Array(repeating: nil, count: stemCount)
        var stemPaths: [String] = []
        if let directory = stemsDirectory {
            for (index, name) in ScoreManifest.stems.enumerated() {
                let path = (directory as NSString).appendingPathComponent("\(name).wav")
                stemWriters[index] = try WavWriter(path: path, sampleRate: Int(sampleRate))
                stemPaths.append(path)
            }
        }
        var highpassL = Biquad.highpass(frequency: 24, q: 0.7, sampleRate: sampleRate), highpassR = highpassL
        var lowShelfL = Biquad.lowShelf(frequency: 120, gainDb: manifest.tiltLowDb, q: 0.7, sampleRate: sampleRate), lowShelfR = lowShelfL
        var highShelfL = Biquad.highShelf(frequency: 6500, gainDb: manifest.tiltHighDb, q: 0.7, sampleRate: sampleRate), highShelfR = highShelfL
        var tensionL = [SVF](repeating: SVF(), count: stemCount), tensionR = [SVF](repeating: SVF(), count: stemCount)
        var tensionLane = manifest.tension
        let stemFX: [StemFX?] = ScoreManifest.stems.map { name in manifest.stemFX[name].map { StemFX(spec: $0, sampleRate: sampleRate) } }
        let spaceStem = ScoreManifest.stems.firstIndex(of: "space")!
        let filtered = ["bass", "harmony", "lead", "texture"].compactMap { ScoreManifest.stems.firstIndex(of: $0) }
        var glue = GlueCompressor(thresholdDb: manifest.glueThresholdDb, ratio: manifest.glueRatio, attack: 0.02, release: 0.25, sampleRate: sampleRate)
        let limiter = Limiter(ceilingDb: manifest.ceilingDb, sampleRate: sampleRate)
        let meter = LoudnessMeter(sampleRate: sampleRate)
        let masterGain = dbToGain(manifest.masterGainDb)
        // Tape: a short modulated delay (wow 0.55 Hz, flutter 6.3 Hz), warm
        // saturation and a gentle high cut that deepens with the amount.
        let tape = clamp01(manifest.tape)
        let tapeL = DelayLine(maxSamples: Int(0.012 * sampleRate)), tapeR = DelayLine(maxSamples: Int(0.012 * sampleRate))
        var tapeToneL = OnePole(), tapeToneR = OnePole()
        tapeToneL.setLowpass(cutoff: 19000 - 7000 * tape, sampleRate: sampleRate); tapeToneR = tapeToneL
        var tapeSample = 0
        var limitedL: [Float] = [], limitedR: [Float] = []
        var emitted = 0  // limiter samples consumed (index relative to firstSample)
        var nonFinite = 0
        let skip = outputFirst - firstSample
        var written = 0
        let wanted = lastSample - outputFirst

        func drainLimited() {
            let n = limitedL.count
            let begin = max(0, skip - emitted)
            let stop = min(n, skip + wanted - emitted)
            if stop > begin {
                limitedL.withUnsafeBufferPointer { lb in
                    limitedR.withUnsafeBufferPointer { rb in
                        writer.write(lb.baseAddress! + begin, rb.baseAddress! + begin, count: stop - begin)
                        meter.process(Array(lb[begin..<stop]), Array(rb[begin..<stop]))
                    }
                }
                written += stop - begin
            }
            emitted += n
            limitedL.removeAll(keepingCapacity: true)
            limitedR.removeAll(keepingCapacity: true)
        }

        // Voices still sounding at renderStart are rebuilt and fast-forwarded,
        // so a window renders exactly like the same slice of the full piece.
        var active: [(voice: Voice, instrument: Int)] = []
        var cursor = 0
        var voiceCount = 0
        var maxPolyphony = 0
        while cursor < manifest.events.count && Int((manifest.events[cursor].start * sr).rounded()) < firstSample {
            let event = manifest.events[cursor]
            cursor += 1
            let spec = manifest.instruments[event.instrument]
            if event.start + event.duration + tail(spec) < renderStart { continue }
            let voice = try makeVoice(event)
            var remaining = firstSample - Int((event.start * sr).rounded())
            var alive = true
            var position = Int((event.start * sr).rounded())
            while remaining > 0 && alive {
                let count = min(block, remaining)
                scratchL.update(repeating: 0, count: count)
                scratchR.update(repeating: 0, count: count)
                var control = InstrumentControl()
                control.time = Float(Double(position) / sr)
                alive = voice.render(scratchL, scratchR, count, control)
                remaining -= count
                position += count
            }
            if alive { active.append((voice, event.instrument)); voiceCount += 1 }
        }

        var sample = firstSample
        while sample < lastSample {
            let count = min(block, lastSample - sample)
            let time = Float(Double(sample) / sr)
            for runtime in runtimes {
                runtime.bufL.update(repeating: 0, count: count)
                runtime.bufR.update(repeating: 0, count: count)
            }
            // Delay timing follows the tempo segments; units alternate per segment.
            while delaySegment + 1 < manifest.delaySegments.count && manifest.delaySegments[delaySegment + 1].time <= Double(time) {
                delaySegment += 1
                let segment = manifest.delaySegments[delaySegment]
                let unit = delays[delaySegment % 2]
                unit.timeL = segment.left * sampleRate
                unit.timeR = segment.right * sampleRate
                unit.smoothTimeL = unit.timeL
                unit.smoothTimeR = unit.timeR
                unit.feedback = segment.feedback
                unit.configure(tone: segment.tone)
            }
            // Ongoing voices first, then the events that start inside this block
            // (rendered from their own sample offset).
            var survivors: [(voice: Voice, instrument: Int)] = []
            survivors.reserveCapacity(active.count + 8)
            for entry in active {
                let runtime = runtimes[entry.instrument]
                var control = InstrumentControl()
                control.time = time
                if runtime.cutoffLane != nil { control.cutoff = runtime.cutoffLane!.value(at: time) }
                if runtime.morphLane != nil { control.morph = runtime.morphLane!.value(at: time) }
                if entry.voice.render(runtime.bufL, runtime.bufR, count, control) {
                    survivors.append(entry)
                }
                runtime.lastActive = sample
            }
            while cursor < manifest.events.count {
                let event = manifest.events[cursor]
                let startSample = Int((event.start * sr).rounded())
                if startSample >= sample + count { break }
                cursor += 1
                let voice = try makeVoice(event)
                voiceCount += 1
                let offset = max(0, startSample - sample)
                let runtime = runtimes[event.instrument]
                var control = InstrumentControl()
                control.time = Float(Double(sample + offset) / sr)
                if runtime.cutoffLane != nil { control.cutoff = runtime.cutoffLane!.value(at: time) }
                if runtime.morphLane != nil { control.morph = runtime.morphLane!.value(at: time) }
                if voice.render(runtime.bufL + offset, runtime.bufR + offset, count - offset, control) {
                    survivors.append((voice, event.instrument))
                }
                runtime.lastActive = sample
            }
            active = survivors
            maxPolyphony = max(maxPolyphony, active.count)

            for k in 0..<stemCount {
                stemL[k].update(repeating: 0, count: count)
                stemR[k].update(repeating: 0, count: count)
            }
            for k in 0..<5 {
                sendL[k].update(repeating: 0, count: count)
                sendR[k].update(repeating: 0, count: count)
            }
            let duckStart = manifest.duck?.value(at: time) ?? 0
            let duckEnd = manifest.duck?.value(at: time + Float(count) / sampleRate) ?? 0
            let delayIndex = max(0, delaySegment) % 2
            let roleLevel = manifest.level.map { dbToGain($0.value(at: time)) } ?? 1
            for runtime in runtimes {
                // Chorus/delay tails need a few seconds after the last voice.
                if sample - runtime.lastActive > Int(3 * sampleRate) { continue }
                let spec = runtime.spec
                let l = runtime.bufL, r = runtime.bufR
                if spec.insertHighpass > 0 { for i in 0..<count { l[i] = runtime.insertHPL.highpass(l[i]); r[i] = runtime.insertHPR.highpass(r[i]) } }
                if spec.drive > 0 {
                    // Heavy drives clip hard (amps); make-up restores the level.
                    let k = 1 + spec.drive * 6
                    let makeup = spec.revision == .v1 ? 1 : 1 + (sqrtf(k) - 1) * clamp01(spec.drive / 2)
                    for i in 0..<count { l[i] = saturate(l[i], spec.drive) * makeup; r[i] = saturate(r[i], spec.drive) * makeup }
                }
                if spec.scoop > 0 || spec.presence > 0 {
                    for i in 0..<count {
                        l[i] += -runtime.scoopL.bandpass(l[i]) * spec.scoop * 0.7 + runtime.presL.bandpass(l[i]) * spec.presence * 0.8
                        r[i] += -runtime.scoopR.bandpass(r[i]) * spec.scoop * 0.7 + runtime.presR.bandpass(r[i]) * spec.presence * 0.8
                    }
                }
                if spec.insertLowpass > 0 { for i in 0..<count { l[i] = runtime.cab2L.lowpass(runtime.cabL.lowpass(l[i])); r[i] = runtime.cab2R.lowpass(runtime.cabR.lowpass(r[i])) } }
                if spec.crushBits > 0 || spec.crushRate > 1 {
                    runtime.crusher.process(l, r, count: count, bits: spec.crushBits, rateDivider: spec.crushRate)
                }
                runtime.chorus?.process(l, r, count: count, mix: spec.chorus, time: time)
                var gain = spec.gain * roleLevel
                if runtime.gainLane != nil { gain *= runtime.gainLane!.value(at: time) }
                for i in 0..<count {
                    let u = Float(i) / Float(count)
                    let duck = duckStart + (duckEnd - duckStart) * u
                    var g = gain
                    if spec.duck > 0 && duck > 0 { g *= dbToGain(-manifest.duckDepthDb * spec.duck * duck) }
                    if spec.pump > 0 && !pumpTimes.isEmpty {
                        let t = time + Float(i) / sampleRate
                        if let last = lastHit(pumpTimes, before: t) {
                            let since = t - last
                            g *= 1 - spec.pump * (1 - smoothstep(since / manifest.pumpRelease))
                        }
                    }
                    var a = l[i], b = r[i]
                    if spec.carve > 0 && duck > 0 {
                        let cut = spec.carve * duck
                        a -= runtime.carveL.bandpass(a) * cut
                        b -= runtime.carveR.bandpass(b) * cut
                    }
                    if spec.width != 1 {
                        let mid = (a + b) * 0.5
                        a = mid + (a - mid) * spec.width
                        b = mid + (b - mid) * spec.width
                    }
                    l[i] = a * g
                    r[i] = b * g
                }
                let stem = runtime.stemIndex
                for i in 0..<count { stemL[stem][i] += l[i]; stemR[stem][i] += r[i] }
                for (busIndex, bus) in ["room", "hall", "space", "delay"].enumerated() {
                    var amount = spec.sends[bus] ?? 0
                    if let lane = runtime.sendLanes[bus] { var copy = lane; amount *= copy.value(at: time) }
                    if amount <= 0 { continue }
                    let target = busIndex == 3 ? 3 + delayIndex : busIndex
                    for i in 0..<count {
                        sendL[target][i] += l[i] * amount
                        sendR[target][i] += r[i] * amount
                    }
                }
            }
            // Returns land in the "space" stem; the delays also feed the hall.
            let space = ScoreManifest.stems.firstIndex(of: "space")!
            for k in 0..<2 {
                scratchL.update(repeating: 0, count: count)
                scratchR.update(repeating: 0, count: count)
                delays[k].process(inL: sendL[3 + k], inR: sendR[3 + k], outL: scratchL, outR: scratchR, count: count)
                for i in 0..<count {
                    stemL[space][i] += scratchL[i]
                    stemR[space][i] += scratchR[i]
                    sendL[1][i] += scratchL[i] * 0.25
                    sendR[1][i] += scratchR[i] * 0.25
                }
            }
            for (k, reverb) in reverbs.enumerated() {
                reverb.process(inL: sendL[k], inR: sendR[k], outL: stemL[space], outR: stemR[space], count: count, sample: sample)
            }
            // Bus EQ, saturation and image, then the returns breathe with the kick.
            for k in 0..<stemCount { stemFX[k]?.process(stemL[k], stemR[k], count: count) }
            if manifest.spaceDuck > 0 && !pumpTimes.isEmpty {
                for i in 0..<count {
                    let t = time + Float(i) / sampleRate
                    if let last = lastHit(pumpTimes, before: t) {
                        let g = 1 - manifest.spaceDuck * (1 - smoothstep((t - last) / (manifest.pumpRelease * 1.6)))
                        stemL[spaceStem][i] *= g; stemR[spaceStem][i] *= g
                    }
                }
            }
            // Tension filter: the music opens into drops and closes into builds.
            if tensionLane != nil {
                let open = clamp01(tensionLane!.value(at: time))
                if open < 0.995 {
                    let cutoff = 160 * powf(2, open * 6.9)
                    for k in filtered {
                        tensionL[k].set(cutoff: cutoff, q: 0.85, sampleRate: sampleRate)
                        tensionR[k].set(cutoff: cutoff, q: 0.85, sampleRate: sampleRate)
                        for i in 0..<count { stemL[k][i] = tensionL[k].lowpass(stemL[k][i]); stemR[k][i] = tensionR[k].lowpass(stemR[k][i]) }
                    }
                } else {
                    for k in filtered { _ = tensionL[k].lowpass(0); _ = tensionR[k].lowpass(0) }
                }
            }
            // Drum bus clip: peaks round off, the groove gets denser.
            if manifest.drumClip > 0, let drums = ScoreManifest.stems.firstIndex(of: "drums") {
                let c = manifest.drumClip
                for i in 0..<count {
                    stemL[drums][i] = c * tanhf(stemL[drums][i] / c)
                    stemR[drums][i] = c * tanhf(stemR[drums][i] / c)
                }
            }
            // Master.
            for i in 0..<count {
                var a: Float = 0, b: Float = 0
                for k in 0..<stemCount { a += stemL[k][i]; b += stemR[k][i] }
                if !a.isFinite || !b.isFinite { nonFinite += 1; a = 0; b = 0 }
                a = highShelfL.process(lowShelfL.process(highpassL.process(a)))
                b = highShelfR.process(lowShelfR.process(highpassR.process(b)))
                mixL[i] = a * masterGain
                mixR[i] = b * masterGain
                if manifest.softClip > 0 {
                    // Soft clip before the limiter: rounder peaks, more density.
                    let k = manifest.softClip
                    mixL[i] = mixL[i] * (1 - k) + tanhf(mixL[i]) * k
                    mixR[i] = mixR[i] * (1 - k) + tanhf(mixR[i]) * k
                }
                if tape > 0 {
                    let ts = Float(tapeSample) / sampleRate
                    let wobble = (0.0006 * sinf(twoPi * 0.55 * ts) + 0.00007 * sinf(twoPi * 6.3 * ts + 1.3)) * tape
                    let center = 0.004 * sampleRate
                    tapeL.push(mixL[i]); tapeR.push(mixR[i])
                    let dl = tapeL.read(center + wobble * sampleRate), dr = tapeR.read(center - wobble * 0.8 * sampleRate)
                    mixL[i] = tapeToneL.lowpass(warm(dl, 0.6 * tape))
                    mixR[i] = tapeToneR.lowpass(warm(dr, 0.6 * tape))
                    tapeSample += 1
                }
            }
            glue.process(mixL, mixR, count: count)
            limiter.process(mixL, mixR, count: count, outL: &limitedL, outR: &limitedR)
            drainLimited()
            if !stemWriters.allSatisfy({ $0 == nil }) {
                let begin = max(0, outputFirst - sample)
                if begin < count {
                    for k in 0..<stemCount {
                        stemWriters[k]?.write(stemL[k] + begin, stemR[k] + begin, count: count - begin)
                    }
                }
            }
            sample += count
            if Date().timeIntervalSince(lastProgress) >= 0.25 {
                progress?(Double(sample - firstSample) / Double(max(1, lastSample - firstSample)), Double(sample) / sr)
                lastProgress = Date()
            }
        }
        limiter.flush(outL: &limitedL, outR: &limitedR)
        drainLimited()
        writer.close()
        stemWriters.forEach { $0?.close() }
        progress?(1, end)
        let elapsed = Date().timeIntervalSince(clock)
        let seconds = Double(written) / sr
        return RenderReport(
            schema: reportSchema,
            output: output, start: start, end: end, seconds: seconds, renderSeconds: elapsed,
            realtimeFactor: seconds / max(elapsed, 1e-6), peakDb: Double(20 * log10f(meter.peak + 1e-9)),
            integratedLufs: meter.integrated, glueMaxReductionDb: Double(glue.maxReduction),
            limiterMinGainDb: Double(20 * log10f(limiter.minGain + 1e-9)), voices: voiceCount,
            maxPolyphony: maxPolyphony, nonFinite: nonFinite, stems: stemPaths)
    }

    private var pumpCursor = 0
    private func lastHit(_ times: [Float], before t: Float) -> Float? {
        if pumpCursor >= times.count || (pumpCursor > 0 && times[pumpCursor - 1] > t) { pumpCursor = 0 }
        while pumpCursor < times.count && times[pumpCursor] <= t { pumpCursor += 1 }
        return pumpCursor > 0 ? times[pumpCursor - 1] : nil
    }
}
