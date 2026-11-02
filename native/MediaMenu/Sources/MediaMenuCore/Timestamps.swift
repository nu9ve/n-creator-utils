import Foundation

/// Engine timestamps carry microseconds and UTC; manifests may stop at minutes with an offset.
public enum Timestamps {
    private static func formatter(_ format: String) -> DateFormatter {
        let value = DateFormatter()
        value.locale = Locale(identifier: "en_US_POSIX")
        value.dateFormat = format
        return value
    }
    public static func parse(_ raw: String) -> Date? {
        guard raw.count >= 16 else { return nil }
        // Fractions vary in length and add nothing a person reads.
        let text = raw.replacingOccurrences(of: #"\.\d+"#, with: "", options: .regularExpression)
        return formatter("yyyy-MM-dd'T'HH:mm:ssXXXXX").date(from: text) ?? formatter("yyyy-MM-dd'T'HH:mmXXXXX").date(from: text)
    }
    /// In the viewer's zone and language; anything unreadable is shown as it came.
    public static func label(_ raw: String) -> String {
        parse(raw)?.formatted(date: .abbreviated, time: .shortened) ?? String(raw.prefix(19)).replacingOccurrences(of: "T", with: " ")
    }
}
