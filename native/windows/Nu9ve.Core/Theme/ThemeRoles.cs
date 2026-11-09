namespace Nu9ve.Theme;

// nu9ve-theme-v1 §2 y §3 · origen: n-creator-utils native/MediaMenu/Sources/MediaMenuCore/ThemeColor.swift
// (ThemeRoles). Port literal de la receta de superficies por skin y de los mínimos de texto.
// Divergencia: ninguna en el cálculo. La forma de la ventana en Windows vive en ThemeSkinInfo
// (windows-app-v1 §3): la ventana sigue siendo del sistema, con sus botones a la derecha.
/// <summary>
/// nüün's roles from one HSV seed. Surfaces keep the original blend recipe; text roles are nudged
/// only when they would fall below the contract's minimums, so a seed that already reads well looks
/// exactly as before.
/// </summary>
public sealed record ThemeRoles
{
    public const double DefaultHue = 95.0 / (163.0 * 6.0);
    public const double DefaultSaturation = 163.0 / 255.0;

    public bool Dark { get; }
    public ThemeSkin Skin { get; }
    public ThemeColor Seed { get; }
    public ThemeColor Canvas { get; }
    public ThemeColor Surface { get; }
    public ThemeColor Raised { get; }
    /// <summary>Opaque line color; the UI draws it at the skin's line opacity.</summary>
    public ThemeColor Line { get; }
    public ThemeColor Ink { get; }
    public ThemeColor Muted { get; }
    /// <summary>The seed as a filled shape: selection pills, badges, the primary button.</summary>
    public ThemeColor Accent { get; }
    public ThemeColor OnAccent { get; }
    /// <summary>The accent as text, icon or stroke on any surface.</summary>
    public ThemeColor AccentText { get; }
    /// <summary>Background of a selected row or tab; ink stays at 7:1 on it.</summary>
    public ThemeColor AccentSoft { get; }
    public ThemeColor Danger { get; }
    public ThemeColor Warning { get; }
    public ThemeColor Success { get; }
    public ThemeColor Info { get; }
    /// <summary>Background of a warning banner; ink stays at 7:1 and warning at 4.5:1 on it.</summary>
    public ThemeColor WarningSoft { get; }

    public static ThemeRoles Default(bool dark = false) =>
        new(ThemeSkin.Glass, dark, DefaultHue, DefaultSaturation, 1);

    public static ThemeRoles From(ThemeSkin skin, bool dark, ThemeColor seed)
    {
        var hsv = seed.Hsv;
        return new ThemeRoles(skin, dark, hsv.Hue, hsv.Saturation, hsv.Value);
    }

    /// <summary><paramref name="requestedDark"/> is the user's mode; a skin with a period look of its own overrides it.</summary>
    public ThemeRoles(ThemeSkin skin, bool requestedDark, double hue, double saturation, double brightness)
    {
        var seed = ThemeColor.FromHsv(hue, saturation, brightness);
        var dark = skin.ForcedDark() ?? requestedDark;
        var @base = dark ? ThemeColor.Black : ThemeColor.White;
        ThemeColor Blend(double fraction) => seed.Mixed(@base, fraction);
        Skin = skin;
        Dark = dark;
        Seed = seed;
        ThemeColor preferredInk;
        switch (skin)
        {
            case ThemeSkin.ClassicMac:
                // A Macintosh II with a color card: 1-bit paper and ink; color only in the accent.
                Canvas = ThemeColor.White; Surface = ThemeColor.White; Raised = new(0.93, 0.93, 0.93); Line = ThemeColor.Black;
                preferredInk = ThemeColor.Black;
                break;
            case ThemeSkin.Win9x:
                // nu9ve-theme-v1 §5 · origen: ios-fractos FractosDesign ThemeChrome.swift `Win9x` (face/highlight/shadow).
                Canvas = new(192 / 255.0, 192 / 255.0, 192 / 255.0); Surface = new(223 / 255.0, 223 / 255.0, 223 / 255.0);
                Raised = ThemeColor.White; Line = new(128 / 255.0, 128 / 255.0, 128 / 255.0);
                preferredInk = ThemeColor.Black;
                break;
            case ThemeSkin.Terminal:
            {
                // Base oscura propia; la señal conserva el matiz del color base y sube de luz para leerse.
                Canvas = ThemeColor.Hex("#050806").Mixed(seed, 0.05);
                Surface = ThemeColor.Hex("#0B100C").Mixed(seed, 0.06);
                Raised = ThemeColor.Hex("#121912").Mixed(seed, 0.08);
                var hsv = seed.Hsv;
                var phosphor = ThemeColor.FromHsv(hsv.Hue, Math.Min(hsv.Saturation, 0.55), 1);
                Line = Canvas.Mixed(phosphor, 0.5);
                preferredInk = phosphor.Mixed(ThemeColor.White, 0.15);
                break;
            }
            case ThemeSkin.Gothic:
                // origen: curado src/lib/theme.ts STYLES.gothic.dark (chrome/sheet/text), tinted 4 % by the seed.
                Canvas = ThemeColor.Hex("#09070A").Mixed(seed, 0.04);
                Surface = ThemeColor.Hex("#141015").Mixed(seed, 0.04);
                Raised = ThemeColor.Hex("#1E181F").Mixed(seed, 0.05);
                Line = Canvas.Mixed(seed, 0.45);
                preferredInk = ThemeColor.Hex("#F0E8E6");
                break;
            default:
                Canvas = Blend(dark ? 0.91 : 0.80);
                Surface = Blend(dark ? 0.85 : 0.94);
                Raised = Blend(0.88);
                Line = Blend(dark ? 0.64 : 0.57);
                preferredInk = dark ? new ThemeColor(0.95, 0.95, 0.95) : seed.Mixed(ThemeColor.Black, 0.86);
                break;
        }
        ThemeColor[] backgrounds = [Canvas, Surface, Raised];
        var ink = preferredInk.Readable(backgrounds, 7);
        Ink = ink;
        Muted = Surface.Mixed(ink, 0.68).Readable(backgrounds, 4.5);
        Accent = seed;
        OnAccent = seed.OnColor;
        // A pale seed needs a darker variant to read as text or a checkbox in light mode.
        AccentText = (dark ? seed : seed.Mixed(ThemeColor.Black, 0.48)).Readable(backgrounds, 4.5);
        var soft = Canvas.Mixed(seed, 0.32);
        // Swift: stride(from: 0.28, through: 0, by: -0.04) where ink.contrast(with: soft) < 7.
        for (var step = 0; step <= 7; step++)
        {
            var amount = Math.Max(0, 0.28 - step * 0.04);
            if (ink.Contrast(soft) < 7) soft = Canvas.Mixed(seed, amount);
        }
        AccentSoft = soft;
        // Hues of the system's red, orange, green and blue; the mix with ink keeps each one in the skin.
        ThemeColor State(string hex, params ThemeColor[] extra) =>
            ThemeColor.Hex(hex).Mixed(ink, 0.12).Readable([.. backgrounds, .. extra], 4.5);
        // The banner is tinted first, then the warning text is made to read on it too.
        var orange = ThemeColor.Hex("#E8870E");
        var banner = Surface.Mixed(orange, 0.14);
        foreach (var amount in new[] { 0.12, 0.10, 0.08, 0.06, 0.04, 0.02, 0.0 })
            if (ink.Contrast(banner) < 7) banner = Surface.Mixed(orange, amount);
        WarningSoft = banner;
        Danger = State("#E5372C");
        Warning = State("#E8870E", banner);
        Success = State("#2E9E57");
        Info = State("#2F78D6");
    }

    /// <summary>The line color with the skin's opacity, as ARGB alpha 0…1.</summary>
    public double LineOpacity => Dark ? Skin.LineOpacity().Dark : Skin.LineOpacity().Light;

    /// <summary>Roles by their contract name (nu9ve-theme-v1 §2), for vectors and platform resources.</summary>
    public IReadOnlyDictionary<string, ThemeColor> ByName() => new Dictionary<string, ThemeColor>
    {
        ["canvas"] = Canvas, ["surface"] = Surface, ["raised"] = Raised, ["line"] = Line,
        ["ink"] = Ink, ["muted"] = Muted, ["accent"] = Accent, ["onAccent"] = OnAccent,
        ["accentText"] = AccentText, ["accentSoft"] = AccentSoft,
        ["danger"] = Danger, ["warning"] = Warning, ["success"] = Success, ["info"] = Info,
        ["warningSoft"] = WarningSoft,
    };

    /// <summary>
    /// Windows 9x caption: like Windows 98's appearance settings, the caption takes the user's color,
    /// kept dark enough for white text. Origin: MediaMenu SkinChrome.swift SkinBarBackground.caption.
    /// </summary>
    public (ThemeColor Start, ThemeColor End) Win9xCaption()
    {
        ThemeColor[] white = [ThemeColor.White];
        return (Seed.Mixed(ThemeColor.Black, 0.55).Readable(white, 4.5), Seed.Mixed(ThemeColor.Black, 0.15).Readable(white, 4.5));
    }
}
