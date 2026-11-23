/// Common color roles from a single sRGB base and a skin.
///
/// The base color is the app's: its default and its presets are part of its identity (§7), so this
/// package has no default color of its own.
public struct ThemeRoles: Sendable, Equatable {
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
    /// nu9ve-theme-v1 §2 states, as text or icons: the semantic hue nudged to 4.5:1 on every surface,
    /// never the user's accent, so a red seed cannot make an error look like a selection.
    public let danger: ThemeColor
    public let warning: ThemeColor
    public let success: ThemeColor
    public let info: ThemeColor
    /// Background of a warning banner; ink stays at 7:1 and `warning` at 4.5:1 on it.
    public let warningSoft: ThemeColor

    /// `dark` is the user's mode unless the skin fixes its own.
    public init(skin: ThemeSkin = .glass, dark requested: Bool, hue: Double, saturation: Double, brightness: Double) {
        self.init(skin: skin, dark: requested, base: .hsv(hue, saturation, brightness))
    }

    /// `era` only changes Macintosh and Windows 9x (nu9ve-theme-v1 §4); `.original` is the period's palette.
    public init(skin: ThemeSkin, dark requested: Bool, base: ThemeColor, era: ThemeEra = .original) {
        let dark = skin.forcedDark(era: era) ?? requested
        self.init(dark: dark, base: base, surfaces: ThemeSurfaces(skin: skin, dark: dark, base: base, era: era))
    }

    /// Resolve the same roles for a custom skin or contrast setting.
    public init(dark: Bool, base seed: ThemeColor, surfaces: ThemeSurfaces,
                preferredAccentText: ThemeColor? = nil, mutedInkMix: Double = 0.68,
                accentSoftMix: Double = 0.32, accentSoftStep: Double = 0.04) {
        self.dark = dark
        self.seed = seed
        canvas = surfaces.canvas
        surface = surfaces.surface
        raised = surfaces.raised
        line = surfaces.line
        let backgrounds = [canvas, surface, raised]
        let ink = surfaces.preferredInk.readable(on: backgrounds, minimum: 7)
        self.ink = ink
        muted = surface.mixed(with: ink, amount: mutedInkMix).readable(on: backgrounds, minimum: 4.5)
        accent = seed
        onAccent = seed.onColor
        // A pale seed needs a darker variant to read as text or a checkbox in light mode.
        let accentCandidate = preferredAccentText ?? (dark ? seed : seed.mixed(with: .black, amount: 0.48))
        accentText = accentCandidate.readable(on: backgrounds, minimum: 4.5)
        var soft = canvas.mixed(with: seed, amount: accentSoftMix)
        for amount in stride(from: 0.28, through: 0, by: -accentSoftStep) where ink.contrast(with: soft) < 7 {
            soft = canvas.mixed(with: seed, amount: amount)
        }
        accentSoft = soft
        // Hues of the system's red, orange, green and blue; the mix with ink keeps each one in the skin.
        func state(_ hex: String, also extra: [ThemeColor] = []) -> ThemeColor {
            ThemeColor(hex: hex)!.mixed(with: ink, amount: 0.12).readable(on: backgrounds + extra, minimum: 4.5)
        }
        // The banner is tinted first, then the warning text is made to read on it too.
        let orange = ThemeColor(hex: "#E8870E")!
        var banner = surface.mixed(with: orange, amount: 0.14)
        for amount in [0.12, 0.10, 0.08, 0.06, 0.04, 0.02, 0.0] where ink.contrast(with: banner) < 7 {
            banner = surface.mixed(with: orange, amount: amount)
        }
        warningSoft = banner
        danger = state("#E5372C")
        warning = state("#E8870E", also: [banner])
        success = state("#2E9E57")
        info = state("#2F78D6")
    }
}
