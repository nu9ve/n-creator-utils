namespace Nu9ve.Theme;

// nu9ve-theme-v1 §7 · origen: MediaMenuCore ThemeColor.swift (ThemePreset). Colores sólo: la cruz y las
// marcas de navajas no se usan.
/// <summary>nüün's Swiss presets.</summary>
public sealed record ThemePreset(string Id, string Hex)
{
    public static readonly IReadOnlyList<ThemePreset> All =
    [
        new("amber", "#FFBB5C"),
        new("knife", "#D52B1E"),
        new("steel", "#8A99A8"),
        new("glacier", "#7CC6E6"),
    ];

    public ThemeColor Color => ThemeColor.Hex(Hex);

    public string Name(bool english = false) => Id switch
    {
        "knife" => english ? "Knife" : "Navaja",
        "steel" => english ? "Steel" : "Acero",
        "glacier" => english ? "Glacier" : "Glaciar",
        _ => english ? "Amber" : "Ámbar",
    };
}
