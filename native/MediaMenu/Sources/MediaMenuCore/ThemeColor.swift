import Foundation

// nu9ve-theme-v1 §3 · origen: soundshop Sources/SoundShop/Appearance.swift ThemeColor.
// Divergencia: sin Codable/SwiftUI (Core no importa UI); `mixed` y `hsv` iguales.
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

// nu9ve-theme-v1 §2 y §3 · origen: native/MediaMenu Appearance.swift CreatorPalette (superficies) +
// soundshop readable (texto). Los roles siguen los nombres comunes del contrato.
/// nüün's roles from one HSV seed. Surfaces keep the original blend recipe; text roles are
/// nudged only when they would fall below the contract's minimums, so a seed that already reads
/// well looks exactly as before.
public struct ThemeRoles: Sendable, Equatable {
    public static let defaultHue = 95.0 / (163.0 * 6.0)
    public static let defaultSaturation = 163.0 / 255.0
    public let dark: Bool
    public let seed: ThemeColor
    public let canvas: ThemeColor
    public let surface: ThemeColor
    public let raised: ThemeColor
    /// Opaque line color; the UI draws it at partial opacity.
    public let line: ThemeColor
    public let ink: ThemeColor
    public let muted: ThemeColor
    /// The seed as a filled shape: selection pills, badges, the primary button.
    public let accent: ThemeColor
    public let onAccent: ThemeColor
    /// The accent as text, icon or stroke on any surface.
    public let accentText: ThemeColor
    /// Background of a selected row or tab; ink stays at 7:1 on it.
    public let accentSoft: ThemeColor
    public let skin: ThemeSkin

    /// `dark` is the user's mode; a skin with a period look of its own overrides it.
    public init(skin: ThemeSkin = .glass, dark requested: Bool, hue: Double, saturation: Double, brightness: Double) {
        let seed = ThemeColor.hsv(hue, saturation, brightness)
        let dark = skin.forcedDark ?? requested
        let base = dark ? ThemeColor.black : ThemeColor.white
        func blend(_ fraction: Double) -> ThemeColor { seed.mixed(with: base, amount: fraction) }
        self.skin = skin
        self.dark = dark
        self.seed = seed
        let preferredInk: ThemeColor
        switch skin {
        case .glass, .dither:
            canvas = blend(dark ? 0.91 : 0.80)
            surface = blend(dark ? 0.85 : 0.94)
            raised = blend(0.88)
            line = blend(dark ? 0.64 : 0.57)
            preferredInk = dark ? ThemeColor(0.95, 0.95, 0.95) : seed.mixed(with: .black, amount: 0.86)
        case .classicMac:
            // A Macintosh II with a color card: 1-bit paper and ink; color only in the accent.
            canvas = .white; surface = .white; raised = ThemeColor(0.93, 0.93, 0.93); line = .black
            preferredInk = .black
        case .win9x:
            // nu9ve-theme-v1 §5 · origen: ios-fractos FractosDesign ThemeChrome.swift `Win9x` (face/highlight/shadow).
            canvas = ThemeColor(192 / 255, 192 / 255, 192 / 255); surface = ThemeColor(223 / 255, 223 / 255, 223 / 255)
            raised = .white; line = ThemeColor(128 / 255, 128 / 255, 128 / 255)
            preferredInk = .black
        case .gothic:
            // origen: curado src/lib/theme.ts STYLES.gothic.dark (chrome/sheet/text), tinted 4 % by the seed.
            canvas = ThemeColor(hex: "#09070A")!.mixed(with: seed, amount: 0.04)
            surface = ThemeColor(hex: "#141015")!.mixed(with: seed, amount: 0.04)
            raised = ThemeColor(hex: "#1E181F")!.mixed(with: seed, amount: 0.05)
            line = canvas.mixed(with: seed, amount: 0.45)
            preferredInk = ThemeColor(hex: "#F0E8E6")!
        }
        let backgrounds = [canvas, surface, raised]
        let ink = preferredInk.readable(on: backgrounds, minimum: 7)
        self.ink = ink
        muted = surface.mixed(with: ink, amount: 0.68).readable(on: backgrounds, minimum: 4.5)
        accent = seed
        onAccent = seed.onColor
        // A pale seed needs a darker variant to read as text or a checkbox in light mode.
        accentText = (dark ? seed : seed.mixed(with: .black, amount: 0.48)).readable(on: backgrounds, minimum: 4.5)
        var soft = canvas.mixed(with: seed, amount: 0.32)
        for amount in stride(from: 0.28, through: 0, by: -0.04) where ink.contrast(with: soft) < 7 {
            soft = canvas.mixed(with: seed, amount: amount)
        }
        accentSoft = soft
    }
}

/// nu9ve-theme-v1 §5: the skins nüün offers. Cristal is the everyday look; the rest are optional
/// period skins for people who like to set up their space.
public enum ThemeSkin: String, CaseIterable, Sendable, Identifiable {
    case glass, dither, classicMac, win9x, gothic
    public var id: String { rawValue }
    public var name: String {
        switch self {
        case .glass: return String(localized: "Cristal")
        case .dither: return String(localized: "Grano")
        case .classicMac: return String(localized: "Macintosh")
        case .win9x: return String(localized: "Windows 9x")
        case .gothic: return String(localized: "Gótico")
        }
    }
    public var summary: String {
        switch self {
        case .glass: return String(localized: "Nativo y tranquilo. El de todos los días.")
        case .dither: return String(localized: "El grano del colectivo: dither ordenado y detalles pixel.")
        case .classicMac: return String(localized: "1 bit, rayas y cajas: un Macintosh II con tarjeta de color.")
        case .win9x: return String(localized: "Biseles grises y título con degradado.")
        case .gothic: return String(localized: "Noche, marco doble y ornamento. Lo contrario de la retícula suiza.")
        }
    }
    /// Period skins keep the look of their era whatever the user's mode.
    public var forcedDark: Bool? {
        switch self {
        case .classicMac, .win9x: return false
        case .gothic: return true
        default: return nil
        }
    }
    /// Only the system's window keeps rounded corners; the rest draw their own bar and controls.
    public var usesSquareWindow: Bool { self == .classicMac || self == .win9x || self == .gothic }
    // Measures from the contract's catalog (§5) and Fractos `ThemeMaterial`.
    /// The row with the app's own controls (navigation, disks, settings).
    public var barHeight: Double { self == .gothic ? 36 : 30 }
    /// A period caption above that row, with the title and the window controls. 0: none.
    public var captionHeight: Double {
        switch self {
        case .classicMac: return 20
        case .win9x: return 22
        default: return 0
        }
    }
    public var controlRadius: Double {
        switch self {
        case .glass: return 7
        case .dither: return 4
        case .classicMac: return 6
        case .win9x: return 0
        case .gothic: return 1
        }
    }
    public var panelRadius: Double {
        switch self {
        case .glass: return 10
        case .dither: return 7
        case .classicMac, .win9x: return 0
        case .gothic: return 2
        }
    }
    /// Radius of pills (navigation, steps, badges). Large means a capsule.
    public var pillRadius: Double {
        switch self {
        case .glass: return 999
        case .dither: return 4
        case .classicMac: return 4
        case .win9x: return 0
        case .gothic: return 1
        }
    }
    /// How strongly lines are drawn; period skins use solid rules.
    public var lineOpacity: (light: Double, dark: Double) {
        switch self {
        case .glass, .dither: return (0.55, 0.65)
        case .classicMac, .win9x: return (1, 1)
        case .gothic: return (0.9, 0.9)
        }
    }
    /// Ordered-dither density over the canvas; 0 means none.
    public var grain: Double { self == .dither ? 0.12 : 0 }
}

/// nu9ve-theme-v1 §5 · origen: curado src/lib/dither.ts BAYER y kyu Theme.swift `Dither.threshold`.
public enum Bayer {
    public static let matrix: [Int] = [
        0, 32, 8, 40, 2, 34, 10, 42, 48, 16, 56, 24, 50, 18, 58, 26, 12, 44, 4, 36, 14, 46, 6, 38, 60, 28, 52, 20, 62, 30, 54, 22,
        3, 35, 11, 43, 1, 33, 9, 41, 51, 19, 59, 27, 49, 17, 57, 25, 15, 47, 7, 39, 13, 45, 5, 37, 63, 31, 55, 23, 61, 29, 53, 21,
    ]
    public static func threshold(_ x: Int, _ y: Int) -> Double { (Double(matrix[(y & 7) * 8 + (x & 7)]) + 0.5) / 64 }
    /// Cells of an 8×8 tile that are inked at `density`, optionally fading left to right over `width` cells.
    public static func cells(density: Double, width: Int = 8, fade: Bool = false) -> [(x: Int, y: Int)] {
        var cells: [(x: Int, y: Int)] = []
        for y in 0..<8 {
            for x in 0..<width {
                let d = fade ? density * (1 - Double(x) / Double(max(1, width - 1))) : density
                if d > threshold(x, y) { cells.append((x, y)) }
            }
        }
        return cells
    }
}

/// nu9ve-theme-v1 §7: nüün's Swiss presets. Colors only; the cross and knife brands are not used.
public struct ThemePreset: Sendable, Identifiable, Equatable {
    public let id: String
    public let hex: String
    public static let all = [
        ThemePreset(id: "amber", hex: "#FFBB5C"),
        ThemePreset(id: "knife", hex: "#D52B1E"),
        ThemePreset(id: "steel", hex: "#8A99A8"),
        ThemePreset(id: "glacier", hex: "#7CC6E6"),
    ]
    /// Literal keys so the compiler extracts them for Localizable.strings.
    public var name: String {
        switch id {
        case "knife": return String(localized: "Navaja")
        case "steel": return String(localized: "Acero")
        case "glacier": return String(localized: "Glaciar")
        default: return String(localized: "Ámbar")
        }
    }
    public var color: ThemeColor { ThemeColor(hex: hex)! }
}
