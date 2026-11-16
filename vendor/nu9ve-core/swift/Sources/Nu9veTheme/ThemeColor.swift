import Foundation

/// Opaque, normalized sRGB. Pure so the palette can be tested without AppKit.
public struct ThemeColor: Hashable, Sendable {
    public var red: Double
    public var green: Double
    public var blue: Double
    public init(_ red: Double, _ green: Double, _ blue: Double) {
        self.red = red; self.green = green; self.blue = blue
    }
    public init?(hex: String) {
        let trimmed = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        let value = trimmed.hasPrefix("#") ? String(trimmed.dropFirst()) : trimmed
        guard value.count == 6, value.allSatisfy({ $0.isHexDigit }), let rgb = UInt32(value, radix: 16) else { return nil }
        self.init(Double((rgb >> 16) & 255) / 255, Double((rgb >> 8) & 255) / 255, Double(rgb & 255) / 255)
    }
    public static let black = Self(0, 0, 0)
    public static let white = Self(1, 1, 1)
    public static func gray(_ level: Double) -> Self { Self(level, level, level) }
    public var hex: String { String(format: "#%02X%02X%02X", Int((red * 255).rounded()), Int((green * 255).rounded()), Int((blue * 255).rounded())) }
    /// WCAG relative luminance.
    public var luminance: Double {
        func linear(_ x: Double) -> Double { x <= 0.04045 ? x / 12.92 : pow((x + 0.055) / 1.055, 2.4) }
        return 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
    }
    public func contrast(with other: Self) -> Double { (max(luminance, other.luminance) + 0.05) / (min(luminance, other.luminance) + 0.05) }
    public func mixed(with other: Self, amount: Double) -> Self {
        Self(red + (other.red - red) * amount, green + (other.green - green) * amount, blue + (other.blue - blue) * amount)
    }
    /// Keep the color unchanged when it already reads well; otherwise mix toward the best text polarity.
    public func readable(on backgrounds: [Self], minimum: Double = 4.5) -> Self {
        func score(_ color: Self) -> Double { backgrounds.map { color.contrast(with: $0) }.min() ?? 21 }
        guard score(self) < minimum else { return self }
        let target = score(.black) >= score(.white) ? Self.black : Self.white
        guard score(target) >= minimum else { return target }
        var low = 0.0, high = 1.0
        for _ in 0..<20 {
            let mid = (low + high) / 2
            if score(mixed(with: target, amount: mid)) >= minimum { high = mid } else { low = mid }
        }
        return mixed(with: target, amount: high)
    }
    /// Black or white, whichever reads best on this color.
    public var onColor: Self { contrast(with: .black) >= contrast(with: .white) ? .black : .white }
    /// Hue, saturation and value in 0…1.
    public var hsv: (hue: Double, saturation: Double, value: Double) {
        let maxC = max(red, green, blue), minC = min(red, green, blue), delta = maxC - minC
        var hue = 0.0
        if delta > 0 {
            if maxC == red { hue = ((green - blue) / delta).truncatingRemainder(dividingBy: 6) }
            else if maxC == green { hue = (blue - red) / delta + 2 }
            else { hue = (red - green) / delta + 4 }
            hue /= 6
            if hue < 0 { hue += 1 }
        }
        return (hue, maxC == 0 ? 0 : delta / maxC, maxC)
    }
    public static func hsv(_ hue: Double, _ saturation: Double, _ value: Double) -> Self {
        let h = (hue >= 1 ? 0 : max(0, hue)) * 6, sector = Int(h), fraction = h - floor(h)
        let p = value * (1 - saturation), q = value * (1 - saturation * fraction), t = value * (1 - saturation * (1 - fraction))
        switch sector {
        case 0: return Self(value, t, p)
        case 1: return Self(q, value, p)
        case 2: return Self(p, value, t)
        case 3: return Self(p, q, value)
        case 4: return Self(t, p, value)
        default: return Self(value, p, q)
        }
    }
}
