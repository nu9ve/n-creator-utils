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

    /// `dark` is the resolved mode for this skin.
    public init(skin: ThemeSkin, dark: Bool, base seed: ThemeColor) {
        let base = dark ? ThemeColor.black : ThemeColor.white
        func blend(_ fraction: Double) -> ThemeColor { seed.mixed(with: base, amount: fraction) }
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
}
