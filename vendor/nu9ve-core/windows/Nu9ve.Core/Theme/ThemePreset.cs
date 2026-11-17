namespace Nu9ve.Theme;

// nu9ve-theme-v1 §7 · par de Nu9veTheme ThemePreset.swift (nu9ve/core).
/// <summary>
/// An opaque color with a stable preset identifier. The presets are part of an app's identity
/// (nu9ve-theme-v1 §7): each app declares its own list and names. This library only gives the type,
/// so no app's colors become everyone's. <see cref="Hex"/> must be an opaque sRGB color (#RRGGBB).
/// </summary>
public sealed record ThemePreset(string Id, string Hex)
{
    public ThemeColor Color => ThemeColor.Hex(Hex);
}
