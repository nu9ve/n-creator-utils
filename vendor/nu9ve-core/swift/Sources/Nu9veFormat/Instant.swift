import Foundation

// nu9ve-controls-v1 §4: an engine sends instants in ISO 8601 with a zone; the app shows them in the zone of
// whoever looks. Parsing is done by hand, not with a formatter, so every platform reads the same instants.

/// Instants as engines send them, and how they read in a zone.
public enum Instant {
    /// Seconds since 1970-01-01T00:00:00Z of an ISO 8601 instant, or nil if it is not one.
    ///
    /// Accepts `YYYY-MM-DDTHH:MM`, with `:SS` and a fraction of 1 to 9 digits, a `T` or a space between date and
    /// time, and a zone `Z`, `±HH:MM` or `±HHMM`. A date alone, a time without zone or an impossible date is nil.
    public static func seconds(_ raw: String) -> Double? {
        let text = Array(raw.utf8)
        var i = 0
        func number(_ width: Int) -> Int? {
            guard i + width <= text.count else { return nil }
            var value = 0
            for byte in text[i..<(i + width)] {
                guard byte >= 48, byte <= 57 else { return nil }
                value = value * 10 + Int(byte - 48)
            }
            i += width
            return value
        }
        func take(_ byte: UInt8) -> Bool {
            guard i < text.count, text[i] == byte else { return false }
            i += 1
            return true
        }
        guard let year = number(4), take(45), let month = number(2), take(45), let day = number(2) else { return nil }
        guard take(84) || take(32), let hour = number(2), take(58), let minute = number(2) else { return nil }
        var second = 0
        var fraction = 0.0
        if take(58) {
            guard let s = number(2) else { return nil }
            second = s
            if take(46) {
                var digits = 0
                var value = 0.0
                var scale = 1.0
                while i < text.count, text[i] >= 48, text[i] <= 57 {
                    scale /= 10
                    value += Double(text[i] - 48) * scale
                    digits += 1
                    i += 1
                }
                guard digits >= 1, digits <= 9 else { return nil }
                fraction = value
            }
        }
        var offset = 0
        if take(90) {
            offset = 0
        } else if i < text.count, text[i] == 43 || text[i] == 45 {
            let sign = text[i] == 45 ? -1 : 1
            i += 1
            guard let oh = number(2) else { return nil }
            _ = take(58)
            guard let om = number(2), oh <= 23, om <= 59 else { return nil }
            offset = sign * (oh * 3600 + om * 60)
        } else {
            return nil
        }
        guard i == text.count else { return nil }
        guard (1...12).contains(month), (1...daysIn(month: month, year: year)).contains(day),
              hour <= 23, minute <= 59, second <= 59 else { return nil }
        let days = daysFromCivil(year: year, month: month, day: day)
        let whole = days * 86_400 + hour * 3600 + minute * 60 + second - offset
        return Double(whole) + fraction
    }

    /// The `Date` of an ISO 8601 instant, or nil.
    public static func date(_ raw: String) -> Date? {
        seconds(raw).map { Date(timeIntervalSince1970: $0) }
    }

    /// `YYYY-MM-DD HH:MM` on the wall clock of a zone. It is the same on every platform, so the shared vectors use it
    /// to check zones; what a person reads comes from `label`.
    public static func wallClock(_ seconds: Double, timeZone: TimeZone) -> String {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = timeZone
        let parts = calendar.dateComponents([.year, .month, .day, .hour, .minute], from: Date(timeIntervalSince1970: seconds))
        func two(_ v: Int?) -> String { let t = String(v ?? 0); return t.count >= 2 ? t : "0" + t }
        return "\(parts.year ?? 0)-\(two(parts.month))-\(two(parts.day)) \(two(parts.hour)):\(two(parts.minute))"
    }

    /// What a person reads: the instant in their zone and language, with the system's formatter. What cannot be
    /// read is shown as it came, never empty.
    public static func label(
        _ raw: String,
        date dateStyle: Date.FormatStyle.DateStyle = .abbreviated,
        time timeStyle: Date.FormatStyle.TimeStyle = .shortened,
        locale: Locale = .current,
        timeZone: TimeZone = .current
    ) -> String {
        guard let date = date(raw) else { return raw }
        var style = Date.FormatStyle(date: dateStyle, time: timeStyle)
        style.locale = locale
        style.timeZone = timeZone
        return date.formatted(style)
    }

    static func daysIn(month: Int, year: Int) -> Int {
        switch month {
        case 2: return (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 ? 29 : 28
        case 4, 6, 9, 11: return 30
        default: return 31
        }
    }

    /// Days since 1970-01-01 in the proleptic Gregorian calendar.
    static func daysFromCivil(year: Int, month: Int, day: Int) -> Int {
        let y = month <= 2 ? year - 1 : year
        let era = (y >= 0 ? y : y - 399) / 400
        let yoe = y - era * 400
        let mp = (month + 9) % 12
        let doy = (153 * mp + 2) / 5 + day - 1
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy
        return era * 146_097 + doe - 719_468
    }
}

/// A range of instants: the start included and the end excluded, as the label next to the control says.
public struct InstantRange: Sendable, Equatable, Codable {
    public var start: Double
    public var end: Double

    public init(start: Double, end: Double) {
        self.start = start
        self.end = end
    }

    /// Whether it can be applied: the start before the end.
    public var isValid: Bool { start.isFinite && end.isFinite && start < end }

    /// Whether an instant falls in it: at the start yes, at the end no.
    public func contains(_ seconds: Double) -> Bool { seconds >= start && seconds < end }
}
