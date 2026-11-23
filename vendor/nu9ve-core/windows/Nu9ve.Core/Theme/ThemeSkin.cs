namespace Nu9ve.Theme;

/// <summary>Where a skin sits in an app's set (nu9ve-theme-v1 §5, «Juego de skins por app»).</summary>
public enum ThemeSkinPart
{
    /// <summary>The app's identity skin, its default. Each app draws its own.</summary>
    Original,
    /// <summary>The one skin the app sets against its original. Each app chooses its own.</summary>
    Contrast,
    /// <summary>Macintosh, Windows 9x and Terminal: the same in every app. This library is their source.</summary>
    Common,
    /// <summary>Optional catalog skins an app may add; never replace the three parts above.</summary>
    Extra,
}

/// <summary>
/// Skins this library can draw, with their window metrics (nu9ve-theme-v1 §5). Par de Nu9veTheme ThemeSkin.swift.
/// The three commons are the shared «nu9ve skin». <c>Glass</c> and <c>Gothic</c> are catalog skins; an app uses one
/// as its original or contrast only if it chooses to. An app's own original that is not in the catalog builds its
/// roles with <see cref="ThemeSurfaces"/>. Identifiers are stable and match the Mac (`glass`, `classicMac`, …).
/// </summary>
public enum ThemeSkin { Glass, ClassicMac, Win9x, Terminal, Gothic }

/// <summary>How the skin dresses a Windows window (windows-app-v1 §3).</summary>
public enum WindowBackdrop { Mica, Solid }

// nu9ve-theme-v1 §5 · medidas iguales a Nu9veTheme ThemeSkin.swift (±1 por plataforma).
// Divergencia Windows (windows-app-v1 §3): no hay ventana cuadrada propia ni controles × − + a la
// izquierda; la ventana y sus botones son del sistema y quedan a la derecha. Un skin de época sólo
// pide esquinas cuadradas a DWM y dibuja su franja de título detrás de la TitleBar del sistema.
public static class ThemeSkinInfo
{
    public static readonly IReadOnlyList<ThemeSkin> All = Enum.GetValues<ThemeSkin>();

    /// <summary>The three skins every app offers, in the order Appearance lists them.</summary>
    public static readonly IReadOnlyList<ThemeSkin> Commons = [ThemeSkin.ClassicMac, ThemeSkin.Win9x, ThemeSkin.Terminal];

    /// <summary>True for the three commons; the others are catalog skins an app may pick.</summary>
    public static bool IsCommon(this ThemeSkin skin) => Commons.Contains(skin);

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

    /// <summary>True for the skins whose palette the user can tint (nu9ve-theme-v1 §4 «Época»).</summary>
    public static bool HasEra(this ThemeSkin skin) => skin is ThemeSkin.ClassicMac or ThemeSkin.Win9x;

    /// <summary>The mode this skin fixes with that era, or null when it follows the user's. A tinted era follows the user.</summary>
    public static bool? ForcedDark(this ThemeSkin skin, ThemeEra era) => era == ThemeEra.Tinted && skin.HasEra() ? null : skin.ForcedDark();

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
