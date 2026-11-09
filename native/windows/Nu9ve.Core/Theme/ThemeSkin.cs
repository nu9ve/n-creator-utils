namespace Nu9ve.Theme;

/// <summary>
/// nu9ve-theme-v1 §5: the skins nüün offers. Cristal is the everyday look; the rest are optional
/// period skins. Identifiers are stable and match the Mac app (`glass`, `classicMac`, …).
/// </summary>
public enum ThemeSkin { Glass, ClassicMac, Win9x, Terminal, Gothic }

/// <summary>How the skin dresses a Windows window (windows-app-v1 §3).</summary>
public enum WindowBackdrop { Mica, Solid }

// nu9ve-theme-v1 §5 · origen: MediaMenuCore ThemeColor.swift (ThemeSkin). Medidas iguales (±1 por plataforma).
// Divergencia Windows (windows-app-v1 §3): no hay ventana cuadrada propia ni controles × − + a la
// izquierda; la ventana y sus botones son del sistema y quedan a la derecha. Un skin de época sólo
// pide esquinas cuadradas a DWM y dibuja su franja de título detrás de la TitleBar del sistema.
public static class ThemeSkinInfo
{
    public static readonly IReadOnlyList<ThemeSkin> All = Enum.GetValues<ThemeSkin>();

    public static string Id(this ThemeSkin skin) => skin switch
    {
        ThemeSkin.ClassicMac => "classicMac",
        ThemeSkin.Win9x => "win9x",
        ThemeSkin.Terminal => "terminal",
        ThemeSkin.Gothic => "gothic",
        _ => "glass",
    };

    /// <summary>Unknown or retired ids (e.g. the removed `dither`) fall back to null; callers use the default.</summary>
    public static ThemeSkin? FromId(string? id) => id switch
    {
        "glass" => ThemeSkin.Glass,
        "classicMac" => ThemeSkin.ClassicMac,
        "win9x" => ThemeSkin.Win9x,
        "terminal" => ThemeSkin.Terminal,
        "gothic" => ThemeSkin.Gothic,
        _ => null,
    };

    public static string Name(this ThemeSkin skin, bool english = false) => skin switch
    {
        ThemeSkin.Terminal => "Terminal",
        ThemeSkin.ClassicMac => "Macintosh",
        ThemeSkin.Win9x => "Windows 9x",
        ThemeSkin.Gothic => english ? "Gothic" : "Gótico",
        _ => english ? "Glass" : "Cristal",
    };

    public static string Summary(this ThemeSkin skin, bool english = false) => skin switch
    {
        ThemeSkin.Terminal => english ? "Phosphor, monospaced type and scanlines. Your base color is the signal." : "Fósforo, letra monoespaciada y líneas de barrido. Tu color base es la señal.",
        ThemeSkin.ClassicMac => english ? "1 bit, stripes and boxes: a Macintosh II with a color card." : "1 bit, rayas y cajas: un Macintosh II con tarjeta de color.",
        ThemeSkin.Win9x => english ? "Grey bevels and a gradient caption." : "Biseles grises y título con degradado.",
        ThemeSkin.Gothic => english ? "Night, double frame and ornament. The opposite of the Swiss grid." : "Noche, marco doble y ornamento. Lo contrario de la retícula suiza.",
        _ => english ? "Native and calm. The everyday one." : "Nativo y tranquilo. El de todos los días.",
    };

    /// <summary>Which part of the skin set it is (nu9ve-theme-v1 §5, «Juego de skins por app»).</summary>
    public static string Part(this ThemeSkin skin) => skin switch
    {
        ThemeSkin.Glass => "original",
        ThemeSkin.Gothic => "contrast",
        _ => "common",
    };

    /// <summary>Period skins keep the look of their era whatever the user's mode.</summary>
    public static bool? ForcedDark(this ThemeSkin skin) => skin switch
    {
        ThemeSkin.ClassicMac or ThemeSkin.Win9x => false,
        ThemeSkin.Gothic or ThemeSkin.Terminal => true,
        _ => null,
    };

    public static double ControlRadius(this ThemeSkin skin) => skin switch
    {
        ThemeSkin.Glass => 7, ThemeSkin.ClassicMac => 6, ThemeSkin.Gothic => 1, _ => 0,
    };

    public static double PanelRadius(this ThemeSkin skin) => skin switch
    {
        ThemeSkin.Glass => 10, ThemeSkin.Gothic => 2, _ => 0,
    };

    /// <summary>Radius of pills (navigation, steps, badges). Large means a capsule.</summary>
    public static double PillRadius(this ThemeSkin skin) => skin switch
    {
        ThemeSkin.Glass => 999, ThemeSkin.ClassicMac => 4, ThemeSkin.Gothic => 1, _ => 0,
    };

    public static (double Light, double Dark) LineOpacity(this ThemeSkin skin) => skin switch
    {
        ThemeSkin.Glass => (0.55, 0.65),
        ThemeSkin.Gothic => (0.9, 0.9),
        _ => (1, 1),
    };

    /// <summary>Line weight of panels and controls in DIPs.</summary>
    public static double LineWidth(this ThemeSkin skin) => skin switch
    {
        ThemeSkin.ClassicMac => 1, ThemeSkin.Win9x => 2, ThemeSkin.Gothic => 1, ThemeSkin.Terminal => 1, _ => 1,
    };

    // ----- Windows (windows-app-v1 §3) -----

    /// <summary>Cristal is Mica on Windows 11; period skins paint a solid canvas.</summary>
    public static WindowBackdrop Backdrop(this ThemeSkin skin) => skin == ThemeSkin.Glass ? WindowBackdrop.Mica : WindowBackdrop.Solid;

    /// <summary>Only Cristal keeps Windows 11's rounded window; the rest ask DWM for square corners.</summary>
    public static bool RoundedWindow(this ThemeSkin skin) => skin == ThemeSkin.Glass;

    /// <summary>
    /// Display family by role (nu9ve-theme-v1 §6), resolved to fonts every Windows 11 has. The Mac app uses
    /// Silom/Tahoma/Menlo/Luminari; packaged OFL fonts (Jersey 15, Pixelify Sans, UnifrakturMaguntia) are pending.
    /// </summary>
    public static string DisplayFamily(this ThemeSkin skin) => skin switch
    {
        ThemeSkin.ClassicMac => "Lucida Console, Consolas",
        ThemeSkin.Win9x => "Tahoma, Microsoft Sans Serif",
        ThemeSkin.Terminal => "Cascadia Mono, Consolas",
        ThemeSkin.Gothic => "Palatino Linotype, Georgia",
        _ => "Georgia, Cambria",
    };

    /// <summary>Body and control family. Period skins that are monospaced keep it everywhere; the rest stay legible.</summary>
    public static string UiFamily(this ThemeSkin skin) => skin switch
    {
        ThemeSkin.Terminal => "Cascadia Mono, Consolas",
        ThemeSkin.Win9x => "Tahoma, Segoe UI",
        _ => "Segoe UI Variable Text, Segoe UI",
    };
}
