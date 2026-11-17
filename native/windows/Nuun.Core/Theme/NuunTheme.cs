using Nu9ve.Theme;

namespace Nuun.Core.Theme;

// nu9ve-theme-v1 §5 y §7 · la identidad de nüün en Windows, par de MediaMenuCore/ThemeIdentity.swift.
// Nu9ve.Core sólo da el catálogo de skins y el cálculo de roles; el ámbar, los presets suizos, qué skin
// es el original y cuál el contraste, y los textos de Apariencia son de esta app.
public static class NuunTheme
{
    /// <summary>The amber seed, as HSV, exactly as the Mac app has always defined it.</summary>
    public const double DefaultHue = 95.0 / (163.0 * 6.0);
    public const double DefaultSaturation = 163.0 / 255.0;

    /// <summary>The Swiss presets. Colors only: the cross and the knife marks are not used.</summary>
    public static readonly IReadOnlyList<ThemePreset> Presets =
    [
        new("amber", "#FFBB5C"),
        new("knife", "#D52B1E"),
        new("steel", "#8A99A8"),
        new("glacier", "#7CC6E6"),
    ];

    /// <summary>Cristal in amber, following the system's mode.</summary>
    public static readonly ThemePreferences Defaults = ThemePreferences.For("#FFBB5C", ThemeSkin.Glass);

    public const ThemeSkin Original = ThemeSkin.Glass;
    public const ThemeSkin Contrast = ThemeSkin.Gothic;

    /// <summary>The roles of nüün's default look (Cristal, amber) in either mode.</summary>
    public static ThemeRoles DefaultRoles(bool dark = false) => new(Original, dark, DefaultHue, DefaultSaturation, 1);

    public static ThemeSkinPart Part(ThemeSkin skin) =>
        skin == Original ? ThemeSkinPart.Original
        : skin == Contrast ? ThemeSkinPart.Contrast
        : skin.IsCommon() ? ThemeSkinPart.Common
        : ThemeSkinPart.Extra;

    public static string Name(this ThemePreset preset, bool english = false) => preset.Id switch
    {
        "knife" => english ? "Knife" : "Navaja",
        "steel" => english ? "Steel" : "Acero",
        "glacier" => english ? "Glacier" : "Glaciar",
        _ => english ? "Amber" : "Ámbar",
    };

    /// <summary>The line under each skin in Appearance, in nüün's voice.</summary>
    public static string Summary(this ThemeSkin skin, bool english = false) => skin switch
    {
        ThemeSkin.Terminal => english ? "Phosphor, monospaced type and scanlines. Your base color is the signal." : "Fósforo, letra monoespaciada y líneas de barrido. Tu color base es la señal.",
        ThemeSkin.ClassicMac => english ? "1 bit, stripes and boxes: a Macintosh II with a color card." : "1 bit, rayas y cajas: un Macintosh II con tarjeta de color.",
        ThemeSkin.Win9x => english ? "Grey bevels and a gradient caption." : "Biseles grises y título con degradado.",
        ThemeSkin.Gothic => english ? "Night, double frame and ornament. The opposite of the Swiss grid." : "Noche, marco doble y ornamento. Lo contrario de la retícula suiza.",
        _ => english ? "Native and calm. The everyday one." : "Nativo y tranquilo. El de todos los días.",
    };
}
