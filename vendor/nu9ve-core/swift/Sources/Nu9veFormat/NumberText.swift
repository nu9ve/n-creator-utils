import Foundation

// nu9ve-controls-v1 §3.2: the readout of a value. Built digit by digit, not with a formatter, so every
// platform writes the same text (data/ui-format-vectors-v1.json).

/// Text of numbers and sizes, the same on every platform.
public enum NumberText {
    /// The minus sign of a readout: U+2212, as wide as a digit in most faces.
    public static let minus = "\u{2212}"
    /// Between a number and its unit, so the two never wrap apart.
    public static let unitSpace = "\u{00A0}"
    /// Units written right after the number.
    public static let tightUnits: Set<String> = ["×", "%", "°"]
    /// What a value that is not a number shows.
    public static let notANumber = "—"

    /// The value rounded half away from zero to `fractionDigits`, the way the readout writes it.
    public static func rounded(_ value: Double, fractionDigits: Int) -> Double {
        guard value.isFinite else { return value }
        let scale = Foundation.pow(10, Double(fractionDigits))
        return (value * scale).rounded(.toNearestOrAwayFromZero) / scale
    }

    /// `−6,0 dB`, `440 Hz`, `1,25×`. No grouping of thousands; a rounded zero has no sign.
    public static func format(_ value: Double, fractionDigits: Int, unit: String = "", decimalSeparator: String = ",") -> String {
        guard value.isFinite else { return notANumber }
        let digits = Swift.max(0, Swift.min(9, fractionDigits))
        let scale = Foundation.pow(10, Double(digits))
        let scaled = (abs(value) * scale).rounded(.toNearestOrAwayFromZero)
        guard scaled < 9e15 else { return notANumber }
        let units = UInt64(scaled)
        var body = String(units)
        if digits > 0 {
            if body.count <= digits { body = String(repeating: "0", count: digits - body.count + 1) + body }
            let cut = body.index(body.endIndex, offsetBy: -digits)
            body = String(body[..<cut]) + decimalSeparator + String(body[cut...])
        }
        let sign = value < 0 && units > 0 ? minus : ""
        return sign + body + unitSuffix(unit)
    }

    /// Bytes in decimal units, as the system shows sizes: `999 B`, `1,2 KB`, `12 MB`, `120 GB`.
    /// One decimal under 10, none from 10 on.
    public static func bytes(_ count: Int64, decimalSeparator: String = ",") -> String {
        let units = ["B", "KB", "MB", "GB", "TB", "PB", "EB"]
        let sign = count < 0 ? minus : ""
        let magnitude = count == Int64.min ? Double(Int64.max) : Double(abs(count))
        if magnitude < 1000 { return sign + String(Int64(magnitude)) + unitSpace + "B" }
        var index = 0
        var value = magnitude
        while value >= 1000, index < units.count - 1 { value /= 1000; index += 1 }
        var digits = value < 10 ? 1 : 0
        var shown = rounded(value, fractionDigits: digits)
        if shown >= 10, digits == 1 { digits = 0; shown = rounded(value, fractionDigits: 0) }
        if shown >= 1000, index < units.count - 1 { index += 1; shown = 1; digits = 1 }
        return sign + format(shown, fractionDigits: digits, decimalSeparator: decimalSeparator) + unitSpace + units[index]
    }

    private static func unitSuffix(_ unit: String) -> String {
        guard !unit.isEmpty else { return "" }
        return tightUnits.contains(unit) ? unit : unitSpace + unit
    }
}
