import Nu9veTheme

/// nüün's identity in the suite theme (nu9ve-theme-v1 §7): its default color and its presets.
/// `Nu9veTheme` has neither: each app declares its own, so adopting the package never changes its look.
public enum NuunTheme {
    /// Amber `#FFBB5C`, the default since the first themed build.
    public static let defaultHue = 95.0 / (163.0 * 6.0)
    public static let defaultSaturation = 163.0 / 255.0
    /// The Swiss presets: amber, knife red, steel and glacier.
    public static let presets = [
        ThemePreset(id: "amber", hex: "#FFBB5C"),
        ThemePreset(id: "knife", hex: "#D52B1E"),
        ThemePreset(id: "steel", hex: "#8A99A8"),
        ThemePreset(id: "glacier", hex: "#7CC6E6"),
    ]
}
