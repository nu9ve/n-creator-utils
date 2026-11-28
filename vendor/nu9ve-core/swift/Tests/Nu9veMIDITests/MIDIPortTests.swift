import Foundation
import Testing
@testable import Nu9veMIDI

// The vectors of nu9ve/core `rust/nu9ve-midi` (src tests and tests/*.rs at b4dee11), one by one.

private func m(_ bytes: [UInt8]) -> MIDIMessage { MIDIMessage.parse(bytes)! }
private func events(_ lines: [(UInt64, [UInt8])]) -> [MIDIEvent] { lines.map { MIDIEvent(atMS: $0.0, message: m($0.1)) } }
private func key(_ kind: MIDIKind, _ channel: UInt8, _ number: UInt8) -> MIDIKey { MIDIKey(kind: kind, channel: channel, number: number) }
/// The repository: `data/midi/` holds the profiles and the crate's tests hold its CoreMIDI capture.
private let repository = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
    .deletingLastPathComponent().deletingLastPathComponent()
private let dataFolder = repository.appendingPathComponent("data/midi")
private func fixture(_ name: String) -> Data {
    try! Data(contentsOf: repository.appendingPathComponent("rust/nu9ve-midi/tests/fixtures/\(name)"))
}
private func ddj() -> MIDIProfile { MIDIProfile.bundled().first { $0.id == "pioneer-ddj-sb" }! }
private func ddjText() -> String {
    MIDIProfile.bundledText("pioneer-ddj-sb-v1")!
}
private var capture: [MIDIEvent] { MIDIDetect.parseLog(String(decoding: fixture("ddj-sb-2026-11-22.txt"), as: UTF8.self)) }

@Suite struct MessageAndInput {
    @Test func readsNotesCCsAndPitchBend() {
        let on = m([0x91, 0x0b, 0x7f])
        #expect(on.key.kind == .note && on.key.channel == 1 && on.pressed)
        #expect(!m([0x91, 0x0b, 0x00]).pressed)
        let off = m([0x81, 0x0b, 0x40])
        #expect(off.key.kind == .note && off.value == 0)
        let cc = m([0xb6, 0x1f, 0x40])
        #expect(cc.key == key(.cc, 6, 0x1f))
        let bend = m([0xe0, 0x00, 0x40])
        #expect(bend.key.kind == .pitchBend && bend.value == 8192)
        #expect(MIDIMessage.parse([0xf8]) == nil)
        #expect(MIDIMessage.parse([0xa0, 1, 2]) == nil)
        #expect(MIDIMessage.parse([0xd0, 0x28, 0]) == nil, "channel pressure, as the Launchpad sends")
        #expect(MIDIMessage.parse([0x90, 0x80, 1]) == nil)
        #expect(m([0x90, 0x0c, 1]).key.label == "CH 1 · NOTE 0x0C")
    }

    @Test func encodersReadBothDirections() {
        #expect(MIDIEncoding.offset64.delta(0x3f) == -1)
        #expect(MIDIEncoding.offset64.delta(0x43) == 3)
        #expect(MIDIEncoding.twosComplement.delta(0x01) == 1)
        #expect(MIDIEncoding.twosComplement.delta(0x7e) == -2)
        #expect(MIDIEncoding.signBit.delta(0x02) == 2)
        #expect(MIDIEncoding.signBit.delta(0x42) == -2)
    }

    @Test func theDetentIsExactlyTheMiddle() {
        #expect(MIDIValue.normalize(64, bits: 7) == 0.5)
        #expect(MIDIValue.normalize(8192, bits: 14) == 0.5)
        #expect(MIDIValue.normalize(0, bits: 7) == 0)
        #expect(MIDIValue.normalize(127, bits: 7) == 1)
        #expect(MIDIValue.normalize(16383, bits: 14) == 1)
        #expect(MIDIValue.normalize(63, bits: 7) < 0.5 && MIDIValue.normalize(65, bits: 7) > 0.5)
    }

    @Test func modesReadAsSaved() throws {
        let relative = try JSONDecoder().decode(MIDIMode.self, from: Data(#"{"type":"relative","encoding":"twosComplement"}"#.utf8))
        #expect(relative == .relative(.twosComplement))
        #expect(String(decoding: try JSONEncoder().encode(MIDIMode.button), as: UTF8.self) == #"{"type":"button"}"#)
    }

    @Test func theGateHoldsLoneStepsAndFollowsAHand() {
        var gate = MIDIGate()
        let step = 64.0 / 16383
        var passed: [Bool] = []
        for (x, t) in [(0.5, UInt64(0)), (0.5 + step, 1000), (0.5, 2000), (0.5 + 2 * step, 3000)] {
            passed.append(gate.pass(x, nowMS: t, deadband: 0.006))
        }
        #expect(passed == [true, false, false, true], "the first sets the anchor, lone steps are noise, drift arrives once")
        var hand: [Bool] = []
        for i in UInt64(1)..<20 { hand.append(gate.pass(0.5 + Double(i) * 0.0001, nowMS: 5000 + i * 5, deadband: 0.006)) }
        #expect(hand.allSatisfy { $0 })
    }
}

@Suite struct Detection {
    @Test func aNotePressedAndReleasedIsAButton() {
        let d = MIDIDetect.classify(events([(0, [0x90, 0x0b, 0x7f]), (90, [0x90, 0x0b, 0x00])]), noisy: [], want: nil)!
        #expect(d.mode == .button && d.key.number == 0x0b)
    }

    @Test func aModifierHeldFirstLosesToThePad() {
        let d = MIDIDetect.classify(events([(0, [0x90, 0x3f, 0x7f]), (700, [0x97, 0x0a, 0x7f]), (850, [0x97, 0x0a, 0x00]), (900, [0x90, 0x3f, 0x00])]),
                                    noisy: [], want: nil)!
        #expect(d.key.channel == 7 && d.key.number == 0x0a)
    }

    @Test func pairsMakeFourteenBits() {
        let d = MIDIDetect.classify(events([(0, [0xb0, 0x13, 0x7f]), (0, [0xb0, 0x33, 0x40]), (3, [0xb0, 0x13, 0x7e]),
                                            (3, [0xb0, 0x33, 0x50]), (5, [0xb0, 0x13, 0x00]), (5, [0xb0, 0x33, 0x00])]),
                                    noisy: [], want: nil)!
        #expect(d.key.number == 0x13 && d.lsb == 0x33 && d.bits == 14)
        #expect(d.min == 0 && d.max == 0x7f << 7 | 0x40)
    }

    @Test func encodersAreToldFromKnobs() {
        let jog = (0..<12).map { (UInt64($0) * 5, [UInt8(0xb0), 0x21, 0x3f]) }
        #expect(MIDIDetect.classify(events(jog), noisy: [], want: nil)!.mode == .relative(.offset64))
        let browse: [UInt8] = [0x7e, 0x7e, 0x7f, 0x01, 0x04, 0x03, 0x01]
        let turns = browse.enumerated().map { (UInt64($0.offset) * 40, [UInt8(0xb6), 0x40, $0.element]) }
        #expect(MIDIDetect.classify(events(turns), noisy: [], want: nil)!.mode == .relative(.twosComplement))
        let knob = (UInt8(60)..<70).map { (UInt64($0) * 8, [UInt8(0xb0), 0x05, $0]) }
        #expect(MIDIDetect.classify(events(knob), noisy: [], want: nil)!.mode == .absolute)
    }

    @Test func noisyControlsAndMirrorsStepAside() {
        var lines: [(UInt64, [UInt8])] = [(0, [0xb1, 0x00, 0x45]), (0, [0xb1, 0x20, 0x10])]
        for i in UInt8(0)..<4 {
            let at = 10 + UInt64(i) * 3
            lines += [(at, [0xb5, 0x02, 0x70 - i]), (at, [0xb5, 0x22, 0x00]), (at + 1, [0xb5, 0x04, 0x70 - i]), (at + 1, [0xb5, 0x24, 0x00])]
        }
        for i in UInt8(0)..<6 { lines.append((30 + UInt64(i), [0xb1, 0x00, 0x45 + i % 2])) }
        let d = MIDIDetect.classify(events(lines), noisy: [key(.cc, 1, 0)], want: nil)!
        #expect(d.key.channel == 5 && d.key.number == 0x02 && d.lsb == 0x22)
        #expect(d.mirrors == [key(.cc, 5, 0x04)])
    }

    @Test func askingForAButtonFindsTheTouchUnderTheTurn() {
        var lines: [(UInt64, [UInt8])] = [(0, [0x90, 0x35, 0x7f])]
        lines += (1..<20).map { (UInt64($0) * 5, [UInt8(0xb0), 0x23, 0x3e]) }
        lines.append((120, [0x90, 0x35, 0x00]))
        let all = events(lines)
        #expect(MIDIDetect.classify(all, noisy: [], want: nil)!.key.number == 0x23)
        let touch = MIDIDetect.classify(all, noisy: [], want: .button)!
        #expect(touch.key.number == 0x35 && touch.mode == .button)
    }

    @Test func theExplorerLearnsNoiseThenClosesGestures() {
        var explorer = MIDIExplorer(quietMS: 400)
        explorer.startBaseline(nowMS: 0, lengthMS: 3000)
        let noise = m([0xb1, 0x00, 0x45])
        explorer.feed(atMS: 1000, noise)
        #expect(explorer.noisy.contains(noise.key))
        explorer.feed(atMS: 4000, noise)
        #expect(!explorer.gestureInProgress)
        explorer.feed(atMS: 5000, m([0x90, 0x0b, 0x7f]))
        #expect(explorer.poll(nowMS: 5600, want: nil) == nil, "a held button waits for what it modifies")
        explorer.feed(atMS: 5700, m([0x90, 0x0b, 0x00]))
        #expect(explorer.poll(nowMS: 5700, want: nil)?.key.number == 0x0b)
        for i in UInt64(0)..<5 { explorer.feed(atMS: 6000 + i * 5, m([0xb0, 0x21, 0x3f])) }
        #expect(explorer.poll(nowMS: 6200, want: nil) == nil, "not quiet long enough")
        #expect(explorer.poll(nowMS: 6500, want: nil)?.key.number == 0x21)
        explorer.feed(atMS: 7000, m([0x91, 0x17, 0x00]))
        #expect(explorer.poll(nowMS: 7000, want: nil) == nil)
        #expect(explorer.seen.contains { $0.noisy && $0.key == noise.key })
    }
}

@Suite struct DDJSBCapture {
    @Test func embeddedProfilesAreTheFilesInData() throws {
        for name in ["pioneer-ddj-sb-v1", "novation-launchpad-pro-v2"] {
            let file = try String(contentsOf: dataFolder.appendingPathComponent("\(name).json"), encoding: .utf8)
            #expect(MIDIProfile.bundledText(name) == file, "run scripts/embed-midi-profiles.py")
        }
    }

    @Test func fixturesAreTheCrateBytes() {
        #expect(capture.count == 1165)
        #expect(capture.first?.atMS == 21_356)
    }

    @Test func coremidiCaptureCoversFaderJogsAndButtons() {
        let all = capture
        func find(_ kind: MIDIKind, _ channel: UInt8, _ number: UInt8, lsb: UInt8? = nil) -> MIDIDetected {
            let control = key(kind, channel, number)
            let selected = all.filter { e in e.message.key == control || (lsb.map { e.message.key == key(.cc, channel, $0) } ?? false) }
            return MIDIDetect.classify(selected, noisy: [], want: nil)!
        }
        let fader = find(.cc, 0, 0x13, lsb: 0x33)
        #expect(fader.mode == .absolute && fader.lsb == 0x33 && fader.bits == 14)
        for deck in UInt8(0)..<2 {
            for encoder in [UInt8(0x21), 0x23] { #expect(find(.cc, deck, encoder).mode == .relative(.offset64)) }
            #expect(find(.note, deck, 0x35).mode == .button)
        }
        #expect(find(.note, 1, 0x0b).mode == .button)
        #expect(find(.note, 7, 0x02).mode == .button)
    }

    @Test func profileAgreesWithCoremidiGesturesAndBrowsePress() {
        let profile = ddj()
        let found = MIDIDetect.replay(capture, baselineMS: 10_000, quietMS: 400)
        for (_, d) in found {
            let control = profile.control(key: d.key)
            #expect(control != nil, "\(d.key.label) is missing from the profile")
            #expect(control?.mode == d.mode && control?.lsb == d.lsb && control?.verified == true)
        }
        func find(_ kind: MIDIKind, _ channel: UInt8, _ number: UInt8) -> MIDIDetected? {
            found.map(\.1).first { $0.key == key(kind, channel, number) }
        }
        #expect(find(.note, 1, 0x0b)?.mode == .button)
        #expect(find(.note, 6, 0x41)?.mode == .button)
        #expect(find(.note, 6, 0x46)?.mode == .button)
        #expect(find(.cc, 6, 0x40)?.mode == .relative(.twosComplement))
        let fx = find(.cc, 5, 0x02)
        #expect(fx?.lsb == 0x22)
        #expect(fx?.mirrors == [key(.cc, 5, 0x04), key(.cc, 5, 0x06)])
    }

    @Test func replayMatchesTheOriginalReaderIncludingInitialSilence() throws {
        let expected = try JSONSerialization.jsonObject(with: fixture("ddj-sb-2026-11-22-replay.json")) as! NSArray
        let actual = MIDIDetect.replay(capture, baselineMS: 10_000, quietMS: 400).map { [NSNumber(value: $0.0), $0.1.jsonObject] as NSArray } as NSArray
        let normalize = { (value: Any) in try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]) }
        #expect(try normalize(actual) == normalize(expected))
    }

    @Test func capturedTempoFaderKeepsTheNormalizationAndNoiseGateResults() {
        let tempo = ddj().control("deck-2-tempo")!
        var lsb = tempo.key; lsb.number = tempo.lsb!
        var gate = MIDIGate()
        var msb: UInt16 = 0, count = 0, accepted = 0, sum: UInt64 = 0
        for event in capture {
            if event.message.key == tempo.key { msb = event.message.value }
            else if event.message.key == lsb {
                let raw = MIDIValue.combineCC14(msb, event.message.value)
                count += 1
                if gate.pass(MIDIValue.normalize(raw, bits: 14), nowMS: event.atMS, deadband: tempo.deadband!) { accepted += 1; sum += UInt64(raw) }
            }
        }
        #expect(count == 208 && accepted == 206 && sum == 2_362_047)
    }
}

@Suite struct Profiles {
    @Test func theDDJSBProfileKeepsTheAddressInventoryAndEvidence() {
        let profile = ddj()
        #expect(profile.controls.count == 125)
        #expect(profile.controls.filter(\.verified).count == 89)
        #expect(profile.matches(port: "PIONEER DDJ-SB MIDI") && profile.matches(port: "pioneer ddj-sb") && !profile.matches(port: "Some Keyboard"))
        for (id, number) in [("browse-press", UInt8(0x41)), ("center-unidentified-note-65", 0x65)] {
            let control = profile.control(id)!
            #expect(control.key == key(.note, 6, number) && control.verified && control.identified == (id == "browse-press"))
        }
        #expect(profile.control("browse-turn")?.mode == .relative(.twosComplement))
        for deck in ["deck-1", "deck-2"] {
            let tempo = profile.control("\(deck)-tempo")!
            #expect(tempo.number == 0 && tempo.lsb == 0x20 && tempo.deadband == 0.008)
        }
        for unit in ["fx1", "fx2"] {
            for suffix in ["copy", "second-copy"] { #expect(profile.control("\(unit)-level-\(suffix)")?.mirrorOf == "\(unit)-level") }
        }
    }

    @Test func rejectsDuplicateIDsAndOverlappingLSBAddresses() throws {
        var profile = ddj()
        profile.controls[1].id = profile.controls[0].id
        #expect(throws: MIDIProfile.Invalid.self) { try profile.validate() }
        profile = ddj()
        var also = profile.control("deck-1-tempo")!
        also.id = "duplicate-tempo-lsb"; also.number = also.lsb!; also.lsb = nil
        profile.controls.append(also)
        #expect { try profile.validate() } throws: { "\($0)".contains("duplicate primary address") }
    }

    @Test func validatesSchemaAndAcceptsDecimalOrHexMIDINumbers() throws {
        let text = ddjText()
        #expect { try MIDIProfile.fromJSON(Data(text.replacingOccurrences(of: "\"schemaVersion\": 1", with: "\"schemaVersion\": 3").utf8)) }
            throws: { "\($0)".contains("unsupported MIDI profile version") }
        let decimal = try MIDIProfile.fromJSON(Data(text.replacingOccurrences(of: "\"0x33\"", with: "51").utf8))
        #expect(decimal.control("deck-1-channel-fader")?.lsb == 0x33)
        #expect(throws: (any Error).self) { try MIDIProfile.fromJSON(Data(text.replacingOccurrences(of: "\"0x33\"", with: "\"0x80\"").utf8)) }
    }

    @Test func theLaunchpadProfileHasTheProgrammerGridAndLights() {
        let pad = MIDIProfile.bundled().first { $0.id == "novation-launchpad-pro" }!
        #expect(pad.schemaVersion == 2 && pad.controls.count == 96)
        #expect(pad.matches(port: "Launchpad Pro Standalone Port") && !pad.matches(port: "Launchpad Pro Live Port"))
        // Seen on the unit on 2026-11-27.
        #expect(pad.control(key: key(.note, 0, 11))?.id == "pad-8-1")
        #expect(pad.control(key: key(.note, 0, 88))?.id == "pad-1-8")
        #expect(pad.control(key: key(.cc, 0, 91))?.id == "top-1")
        #expect(pad.control(key: key(.cc, 0, 19))?.id == "right-8")
        #expect(pad.controls.allSatisfy { pad.isLit($0) })
        #expect(!ddj().controls.contains { ddj().isLit($0) }, "v1 has no output")
    }
}

@Suite struct Lights {
    private let pad = MIDIProfile.bundled().first { $0.id == "novation-launchpad-pro" }!

    @Test func paletteLightsUseTheProfileColorsAndHardwareMotion() {
        let feedback = pad.feedback!
        let control = pad.control("pad-5-4")!  // note 44: seen flashing on the unit
        #expect(feedback.messages(control, MIDILight(.red)) == [[0x90, 44, 5]])
        #expect(feedback.messages(control, MIDILight(.green, dim: true)) == [[0x90, 44, 23]])
        #expect(feedback.messages(control, MIDILight(.pink, motion: .flash)) == [[0x90, 44, 53], [0xF0, 0x00, 0x20, 0x29, 0x02, 0x10, 0x23, 44, 53, 0xF7]])
        #expect(feedback.messages(pad.control("top-1")!, MIDILight(.blue)) == [[0xb0, 91, 45]])
        #expect(!feedback.blinksInSoftware(MIDILight(.red, motion: .pulse)))
    }

    @Test func theBoardSendsOnlyWhatChangedAndEverythingAfterAReconnect() {
        var board = MIDILightBoard(profile: pad)
        let r1 = board.update(["pad-1-1": MIDILight(.green)])
        #expect(r1 == [[0x90, 81, 21]], "lights never lit are not switched off")
        let r2 = board.update(["pad-1-1": MIDILight(.green)])
        #expect(r2.isEmpty)
        let r3 = board.update(["pad-1-2": MIDILight(.red)])
        #expect(r3 == [[0x90, 81, 0], [0x90, 82, 5]])
        let again = board.forget()
        #expect(again.first == [0xF0, 0x00, 0x20, 0x29, 0x02, 0x10, 0x2C, 0x03, 0xF7])
        #expect(again.count == 1 + 96 + 1, "init, every light off, then what is wanted")
        #expect(again.contains([0xb0, 91, 0]) && again.last == [0x90, 82, 5])
        let r4 = board.darken()
        #expect(r4 == [[0x90, 82, 0]])
    }

    @Test func onOffLightsBlinkInSoftware() throws {
        var profile = pad
        profile.feedback = MIDIFeedback(kind: .onOff, allControls: true, initialize: nil, palette: nil, on: 0x7f, flash: nil, pulse: nil, evidence: nil)
        var board = MIDILightBoard(profile: profile)
        let r5 = board.update(["pad-1-1": MIDILight(.red, motion: .flash)])
        #expect(r5 == [[0x90, 81, 0x7f]])
        #expect(board.needsBlink)
        let r6 = board.blink(phase: false)
        #expect(r6 == [[0x90, 81, 0]])
        _ = board.update(["pad-1-1": MIDILight(.red)])
        #expect(!board.needsBlink)
    }
}

@Suite struct MapsAndReading {
    private let pad = MIDIProfile.bundled().first { $0.id == "novation-launchpad-pro" }!

    /// contracts/nu9ve-controller-map-v1.md §5: the example decodes, and every control it names is in its profile.
    @Test func theContractExampleDecodesAgainstTheProfiles() throws {
        let data = try Data(contentsOf: dataFolder.appendingPathComponent("controller-map-example-v1.json"))
        let file = try MIDIMapFile.decode(data)
        #expect(file.enabled && file.devices.count == 3)
        let profiles = MIDIProfile.bundled()
        for device in file.devices {
            #expect(device.conflicts.isEmpty, "\(device.match)")
            let profile = device.profile.flatMap { id in profiles.first { $0.id == id } }
            #expect((device.profile == nil) == (profile == nil), "\(device.match): unknown profile")
            for mapping in device.mappings {
                #expect(mapping.target != nil, "\(mapping.action): no control and no key")
                if let control = mapping.control { #expect(profile?.control(control) != nil, "\(control)") }
            }
        }
        let launchpad = try #require(file.device(port: "Launchpad Pro Standalone Port"))
        #expect(launchpad.mapping(for: key(.cc, 0, 0x0a), profile: pad)?.action == "recording.toggle")
        #expect(launchpad.mappings.last?.color == .cyan)
        let generic = try #require(file.device(port: "Generic Keys 1"))
        #expect(generic.mapping(for: key(.cc, 15, 7), profile: nil)?.action == "mix.volume")
    }

    @Test func aTargetHoldsOneActionAndSaysWhatItDisplaced() throws {
        var device = MIDIDeviceMap(match: "Launchpad Pro Standalone", profile: pad.id)
        #expect(device.assign(.control("pad-1-1"), action: "scene.select", argument: "a") == nil)
        let displaced = device.assign(.control("pad-1-1"), action: "recording.toggle")
        #expect(displaced?.action == "scene.select")
        #expect(device.mappings.count == 1 && device.conflicts.isEmpty)
        #expect(device.mapping(for: key(.note, 0, 81), profile: pad)?.action == "recording.toggle")
        let file = MIDIMapFile(enabled: true, devices: [device])
        #expect(try MIDIMapFile.decode(file.encoded()) == file)
        #expect(file.device(port: "Launchpad Pro Standalone Port") != nil && file.device(port: "Launchpad Pro Live Port") == nil)
    }

    @Test func rawAddressesMapWithoutAProfile() throws {
        let json = #"{"schemaVersion":1,"enabled":true,"devices":[{"match":"V25","profile":null,"mappings":[{"key":{"kind":"note","channel":1,"number":"0x30"},"action":"preset.next"}]}]}"#
        let file = try MIDIMapFile.decode(Data(json.utf8))
        #expect(file.devices[0].mapping(for: key(.note, 0, 0x30), profile: nil)?.action == "preset.next")
    }

    @Test func theReaderGivesPressesOnceAndJoinsFourteenBits() {
        var reader = MIDIReader(profile: pad)
        let r7 = reader.read(m([0x90, 11, 1]), atMS: 0)
        #expect(r7.map(\.gesture) == [.press], "a light press is a press")
        let r9 = reader.read(m([0x90, 11, 0]), atMS: 5)
        #expect(r9.map(\.gesture) == [.release])
        let r10 = reader.read(m([0x90, 11, 0]), atMS: 6)
        #expect(r10.isEmpty)
        let r11 = reader.read(m([0xb0, 91, 127]), atMS: 7)
        #expect(r11.first?.control == "top-1")

        var dj = MIDIReader(profile: ddj())
        let r12 = dj.read(m([0xb6, 0x1f, 0x40]), atMS: 0)
        #expect(r12.isEmpty, "an MSB waits for its LSB")
        let fade = dj.read(m([0xb6, 0x3f, 0x00]), atMS: 1)
        #expect(fade.first?.control == "crossfader" && fade.first?.gesture == .value(0.5))
        let r13 = dj.read(m([0xb6, 0x40, 0x7f]), atMS: 2)
        #expect(r13.first?.gesture == .step(-1))
        let r14 = dj.read(m([0xb4, 0x04, 0x10]), atMS: 3)
        #expect(r14.isEmpty)
        let r15 = dj.read(m([0xb4, 0x24, 0x00]), atMS: 4)
        #expect(r15.first?.control == "fx1-level", "a mirror reads as its primary")
    }
}
