import Foundation
import Testing
import Nu9veFormat

// The rules of nu9ve-controls-v1 §3 and §4, said one by one. The vectors pin the numbers; these say why.

private let gain = ParamSpec(id: "gain", label: "Ganancia", min: -24, max: 24, step: 0.5, defaultValue: 0, origin: 0, unit: "dB", fractionDigits: 1)
private let frequency = ParamSpec(id: "frequency", label: "Frecuencia", min: 20, max: 20_000, step: 0.01, defaultValue: 1000, scale: .log, unit: "Hz")

@Test func aSpecIsValidOnlyWhenItCanBeUsed() {
    #expect(gain.isValid)
    #expect(frequency.isValid)
    #expect(!ParamSpec(id: "x", label: "x", min: 1, max: 1, step: 1, defaultValue: 1).isValid)
    #expect(!ParamSpec(id: "x", label: "x", min: 0, max: 10, step: 0, defaultValue: 0).isValid)
    #expect(!ParamSpec(id: "x", label: "x", min: 0, max: 10, step: 1, defaultValue: 11).isValid)
    #expect(!ParamSpec(id: "x", label: "x", min: 0, max: 10, step: 0.1, defaultValue: 1, scale: .log).isValid, "log needs min > 0")
    #expect(!ParamSpec(id: "x", label: "x", min: 1, max: 10, step: 2, defaultValue: 1, scale: .log).isValid, "a log step is a fraction")
}

@Test func theOriginDefaultsToTheMinimum() {
    #expect(ParamSpec(id: "x", label: "x", min: 2, max: 8, step: 1, defaultValue: 4).origin == 2)
    #expect(gain.origin == 0)
}

@Test func positionAndValueGoBothWays() {
    for v in [-24.0, -6, 0, 12.5, 24] { #expect(abs(gain.value(at: gain.position(of: v)) - v) < 1e-9) }
    for v in [20.0, 440, 1000, 20_000] { #expect(abs(frequency.value(at: frequency.position(of: v)) - v) / v < 1e-9) }
    #expect(abs(frequency.value(at: 0.5) - 632.4555320336759) < 1e-6, "the middle of 20 Hz–20 kHz is their geometric mean")
}

@Test func valuesStayInsideTheBounds() {
    #expect(gain.clamped(30) == 24)
    #expect(gain.clamped(-30) == -24)
    #expect(gain.clamped(.nan) == 0, "not a number becomes the default")
    #expect(gain.value(at: 2) == 24)
    #expect(gain.stepped(24, .large) == 24)
}

@Test func stepsFollowTheirKind() {
    #expect(gain.stepped(0) == 0.5)
    #expect(gain.stepped(0, count: -1) == -0.5)
    #expect(gain.stepped(0, .large) == 2, "four steps by default")
    #expect(abs(gain.stepped(0, .fine) - 0.05) < 1e-12, "a tenth of a step, off the grid")
    #expect(gain.stepped(0.26) == 1, "a normal step lands on the grid")
}

@Test func aLogStepIsAFractionOfTheTravel() {
    let up = frequency.stepped(1000)
    #expect(abs(frequency.position(of: up) - frequency.position(of: 1000) - 0.01) < 1e-12)
    #expect(up / 1000 == frequency.stepped(100) / 100 || abs(up / 1000 - frequency.stepped(100) / 100) < 1e-9, "same ratio everywhere")
}

@Test func theReadoutWritesDigitsSeparatorAndUnit() {
    #expect(gain.text(-6) == "\u{2212}6,0\u{00A0}dB")
    #expect(gain.text(-6, decimalSeparator: ".") == "\u{2212}6.0\u{00A0}dB")
    #expect(frequency.text(440) == "440\u{00A0}Hz")
    #expect(NumberText.format(1.25, fractionDigits: 2, unit: "×") == "1,25×")
    #expect(NumberText.format(-0.04, fractionDigits: 1) == "0,0", "a rounded zero has no sign")
    #expect(NumberText.format(.nan, fractionDigits: 1) == "—")
    #expect(NumberText.format(5600, fractionDigits: 0, unit: "K") == "5600\u{00A0}K", "no grouping")
    #expect(gain.text(30) == "24,0\u{00A0}dB", "the readout stays inside the bounds")
    #expect(gain.text(.nan) == "—")
}

@Test func theDefaultReadsAsUnchanged() {
    #expect(gain.isDefault(0))
    #expect(!gain.isDefault(0.5))
}

@Test func bytesUseDecimalUnits() {
    #expect(NumberText.bytes(999) == "999\u{00A0}B")
    #expect(NumberText.bytes(1_200_000_000) == "1,2\u{00A0}GB")
    #expect(NumberText.bytes(120_000_000_000) == "120\u{00A0}GB")
    #expect(NumberText.bytes(999_950_000) == "1,0\u{00A0}GB", "rounding up moves to the next unit")
    #expect(NumberText.bytes(9_950) == "10\u{00A0}KB", "from 10 on, no decimal")
}

@Test func timecodesHaveOneShape() {
    #expect(Timecode.format(61.25) == "1:01.25")
    #expect(Timecode.format(3725.5) == "1:02:05.50")
    #expect(Timecode.format(-1.5) == "\u{2212}0:01.50")
    #expect(Timecode.format(.nan) == "--:--.--")
    #expect(Timecode.format(1.039, framesPerSecond: 25) == "0:00:01:00", "the frame the time falls in")
    #expect(Timecode.format(1.04, framesPerSecond: 25) == "0:00:01:01")
    #expect(Timecode.format(10, framesPerSecond: 120) == "0:00:10:000")
}

@Test func instantsReadAsEnginesSendThem() throws {
    #expect(Instant.seconds("1970-01-01T00:00:00Z") == 0)
    #expect(Instant.seconds("2026-11-22T11:30-07:00") == Instant.seconds("2026-11-22T18:30:00Z"))
    let fraction = try #require(Instant.seconds("2026-11-22T18:30:00.123456+00:00"))
    #expect(abs(fraction - (try #require(Instant.seconds("2026-11-22T18:30:00Z"))) - 0.123456) < 1e-6)
    #expect(Instant.seconds("2026-11-22") == nil, "a date alone is not an instant")
    #expect(Instant.seconds("2026-11-22T18:30:00") == nil, "an instant carries its zone")
    #expect(Instant.seconds("2025-02-29T00:00:00Z") == nil)
}

@Test func theWallClockFollowsTheZone() throws {
    let s = try #require(Instant.seconds("2026-11-22T18:30:00Z"))
    #expect(Instant.wallClock(s, timeZone: try #require(TimeZone(identifier: "America/Hermosillo"))) == "2026-11-22 11:30")
    let spring = try #require(Instant.seconds("2026-03-29T01:00:00Z"))
    #expect(Instant.wallClock(spring, timeZone: try #require(TimeZone(identifier: "Europe/Madrid"))) == "2026-03-29 03:00", "summer time")
}

@Test func whatCannotBeReadIsShownAsItCame() {
    #expect(Instant.label("mañana") == "mañana")
    #expect(!Instant.label("2026-11-22T18:30:00Z", locale: Locale(identifier: "es_MX")).isEmpty)
}

@Test func aRangeIncludesItsStartAndNotItsEnd() {
    let range = InstantRange(start: 10, end: 20)
    #expect(range.isValid)
    #expect(range.contains(10))
    #expect(!range.contains(20))
    #expect(!InstantRange(start: 20, end: 20).isValid)
}
