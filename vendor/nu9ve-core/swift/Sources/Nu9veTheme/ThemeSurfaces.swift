/// Neutral surfaces, divider and preferred ink before applying contrast requirements.
public struct ThemeSurfaces: Sendable {
    public var canvas: ThemeColor
    public var surface: ThemeColor
    public var raised: ThemeColor
    public var line: ThemeColor
    public var preferredInk: ThemeColor

    public init(canvas: ThemeColor, surface: ThemeColor, raised: ThemeColor, line: ThemeColor, preferredInk: ThemeColor) {
        self.canvas = canvas
        self.surface = surface
        self.raised = raised
        self.line = line
        self.preferredInk = preferredInk
    }

    /// `dark` is the resolved mode for this skin. `era` matters only for Macintosh and Windows 9x.
    public init(skin: ThemeSkin, dark: Bool, base seed: ThemeColor, era: ThemeEra = .original) {
        let base = dark ? ThemeColor.black : ThemeColor.white
        func blend(_ fraction: Double) -> ThemeColor { seed.mixed(with: base, amount: fraction) }
        if era == .tinted, skin.hasEra {
            self = Self.tinted(skin: skin, dark: dark, seed: seed)
            return
        }
        switch skin {
        case .glass:
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
            canvas = ThemeColor(192 / 255, 192 / 255, 192 / 255); surface = ThemeColor(223 / 255, 223 / 255, 223 / 255)
            raised = .white; line = ThemeColor(128 / 255, 128 / 255, 128 / 255)
            preferredInk = .black
        case .terminal:
            canvas = ThemeColor(hex: "#050806")!.mixed(with: seed, amount: 0.05)
            surface = ThemeColor(hex: "#0B100C")!.mixed(with: seed, amount: 0.06)
            raised = ThemeColor(hex: "#121912")!.mixed(with: seed, amount: 0.08)
            let hsv = seed.hsv
            let phosphor = ThemeColor.hsv(hsv.hue, min(hsv.saturation, 0.55), 1)
            line = canvas.mixed(with: phosphor, amount: 0.5)
            preferredInk = phosphor.mixed(with: .white, amount: 0.15)
        case .gothic:
            canvas = ThemeColor(hex: "#09070A")!.mixed(with: seed, amount: 0.04)
            surface = ThemeColor(hex: "#141015")!.mixed(with: seed, amount: 0.04)
            raised = ThemeColor(hex: "#1E181F")!.mixed(with: seed, amount: 0.05)
            line = canvas.mixed(with: seed, amount: 0.45)
            preferredInk = ThemeColor(hex: "#F0E8E6")!
        }
    }

    /// nu9ve-theme-v1 §5, era `tinted`: the period's structure with the user's color and mode.
    /// Macintosh keeps paper, a solid rule and ink; Windows 9x keeps its face, highlight and shadow.
    private static func tinted(skin: ThemeSkin, dark: Bool, seed: ThemeColor) -> Self {
        if skin == .classicMac {
            let paper = dark ? ThemeColor.black : ThemeColor.white
            let ink = dark ? ThemeColor.white : ThemeColor.black
            let canvas = seed.mixed(with: paper, amount: 0.90), surface = seed.mixed(with: paper, amount: dark ? 0.84 : 0.96)
            let raised = seed.mixed(with: paper, amount: dark ? 0.76 : 0.84)
            // The pinstripes, rules and frames carry the user's color (2026-11-26): 65 % of the seed, at least 3:1 on
            // the paper, the minimum for graphics (3.1 before rounding to 8 bits). Ink stays near black (or white) for text.
            let line = seed.mixed(with: ink, amount: 0.35).readable(on: [canvas, surface, raised], minimum: 3.1)
            return Self(canvas: canvas, surface: surface, raised: raised, line: line,
                        preferredInk: seed.mixed(with: ink, amount: dark ? 0.92 : 0.90))
        }
        // Windows 9x: raised is the bevel's highlight and line its light shadow, so they stay lighter and darker than the face.
        if dark {
            return Self(canvas: ThemeColor.gray(0.16).mixed(with: seed, amount: 0.16), surface: ThemeColor.gray(0.20).mixed(with: seed, amount: 0.12),
                        raised: ThemeColor.gray(0.27).mixed(with: seed, amount: 0.08), line: ThemeColor.gray(0.06).mixed(with: seed, amount: 0.08),
                        preferredInk: .white)
        }
        return Self(canvas: ThemeColor.gray(200 / 255).mixed(with: seed, amount: 0.22), surface: ThemeColor.gray(225 / 255).mixed(with: seed, amount: 0.16),
                    raised: ThemeColor.white.mixed(with: seed, amount: 0.06), line: ThemeColor.gray(128 / 255).mixed(with: seed, amount: 0.28),
                    preferredInk: .black)
    }
}
