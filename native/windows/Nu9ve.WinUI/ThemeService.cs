using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Nu9ve.Theme;
using Windows.UI.ViewManagement;

namespace Nu9ve.WinUI;

/// <summary>
/// nu9ve-theme-v1 on WinUI. One instance per process: computes <see cref="ThemeRoles"/> from the user's
/// choice and writes them into shared brushes (`Nu.ink`, `Nu.surface`…). Views hold those brush instances,
/// so a change of color repaints without rebuilding anything; a change of skin raises <see cref="Changed"/>
/// so skinned controls redraw their shape. In Windows high contrast the system colors win (windows-app-v1 §3).
/// </summary>
public sealed class ThemeService
{
    static ThemeService? current;
    /// <summary>Created on first use, after the static tables below (static initializers run in order).</summary>
    public static ThemeService Current => current ??= new();

    readonly UISettings ui = new();
    readonly AccessibilitySettings accessibility = new();
    readonly Dictionary<string, SolidColorBrush> brushes = [];
    Microsoft.UI.Dispatching.DispatcherQueue? dispatcher;

    public ThemePreferences Preferences { get; private set; } = ThemePreferences.Default;
    public ThemeRoles Roles { get; private set; } = ThemeRoles.Default();
    public bool HighContrast { get; private set; }
    /// <summary>«Efectos de animación» of Windows (Reducir movimiento): decorative motion stops when off.</summary>
    public bool AnimationsEnabled => ui.AnimationsEnabled;
    /// <summary>«Efectos de transparencia»: Mica falls back to a solid canvas when off.</summary>
    public bool TransparencyEnabled => ui.AdvancedEffectsEnabled;

    /// <summary>Raised on the UI thread after roles changed. `skinChanged` tells shapes and fonts to redraw.</summary>
    public event Action<bool>? Changed;

    static readonly string[] RoleNames =
        ["canvas", "surface", "raised", "line", "ink", "muted", "accent", "onAccent", "accentText", "accentSoft",
         "danger", "warning", "success", "info", "warningSoft", "backdrop"];

    // WinUI keys that built-in controls (TextBox, Slider, ToggleSwitch, ComboBox, ContentDialog) read.
    // Overridden at the top of Application.Resources with the same brush instances. Best effort: a key that a
    // control template resolves inside XamlControlsResources keeps the system color (noted in STATUS).
    static readonly (string Key, string Role)[] SystemKeys =
    [
        ("TextFillColorPrimaryBrush", "ink"), ("TextFillColorSecondaryBrush", "muted"), ("TextFillColorTertiaryBrush", "muted"),
        ("AccentFillColorDefaultBrush", "accent"), ("AccentFillColorSecondaryBrush", "accent"), ("AccentFillColorTertiaryBrush", "accent"),
        ("TextOnAccentFillColorPrimaryBrush", "onAccent"), ("AccentTextFillColorPrimaryBrush", "accentText"),
        ("AccentTextFillColorSecondaryBrush", "accentText"), ("CardBackgroundFillColorDefaultBrush", "surface"),
        ("SolidBackgroundFillColorBaseBrush", "canvas"), ("ContentDialogBackground", "surface"),
        ("ControlFillColorInputActiveBrush", "raised"), ("TextControlBackgroundFocused", "raised"),
        // Control-specific keys: overriding the base accent alone does not reach them (they resolve inside
        // XamlControlsResources), so the ones nüün uses are named here.
        ("SliderTrackValueFill", "accent"), ("SliderTrackValueFillPointerOver", "accent"), ("SliderTrackValueFillPressed", "accent"),
        ("SliderThumbBackground", "accent"), ("SliderThumbBackgroundPointerOver", "accent"), ("SliderThumbBackgroundPressed", "accent"),
        ("TextControlBorderBrushFocused", "accentText"), ("ToggleSwitchFillOn", "accent"), ("ToggleSwitchFillOnPointerOver", "accent"),
        ("ProgressBarForeground", "info"), ("ExpanderHeaderBackground", "surface"), ("ExpanderContentBackground", "surface"),
    ];

    ThemeService()
    {
        foreach (var name in RoleNames) brushes[name] = new SolidColorBrush();
    }

    public SolidColorBrush Brush(string role) => brushes[role];

    public static Windows.UI.Color Color(ThemeColor color, double opacity = 1)
    {
        var (r, g, b) = color.Bytes;
        return Windows.UI.Color.FromArgb((byte)Math.Round(Math.Clamp(opacity, 0, 1) * 255), r, g, b);
    }

    /// <summary>A new brush for a one-off color (a surface's fill or edge). Shared roles use <see cref="Brush"/>.</summary>
    public static SolidColorBrush NewBrush(ThemeColor color, double opacity = 1) => new(Color(color, opacity));

    /// <summary>Before any window: registers the brushes and loads the saved choice.</summary>
    public void Install(ResourceDictionary resources)
    {
        foreach (var (name, brush) in brushes) resources[$"Nu.{name}"] = brush;
        foreach (var (key, role) in SystemKeys) resources[key] = brushes[role];
        Preferences = ThemePreferences.Parse(Storage.Read("theme.json"));
        HighContrast = accessibility.HighContrast;
        Recompute();
    }

    /// <summary>Starts following the system's mode, contrast and effects. Call once on the UI thread.</summary>
    public void Follow(Microsoft.UI.Dispatching.DispatcherQueue queue)
    {
        dispatcher = queue;
        // AccessibilitySettings.HighContrastChanged needs a CoreWindow and throws 0x80070490 in a desktop app.
        // Turning high contrast on or off also raises UISettings.ColorValuesChanged, so it is read again there.
        ui.ColorValuesChanged += (_, _) => dispatcher?.TryEnqueue(() =>
        {
            var wasHigh = HighContrast;
            HighContrast = accessibility.HighContrast;
            Recompute();
            Changed?.Invoke(wasHigh != HighContrast);
        });
        try
        {
            ui.AdvancedEffectsEnabledChanged += (_, _) => dispatcher?.TryEnqueue(() => Changed?.Invoke(true));
            ui.AnimationsEnabledChanged += (_, _) => dispatcher?.TryEnqueue(() => Changed?.Invoke(false));
        }
        catch (System.Runtime.InteropServices.COMException) { /* read again on the next change of mode */ }
    }

    public bool SystemDark
    {
        get
        {
            var background = ui.GetColorValue(UIColorType.Background);
            return background.R + background.G + background.B < 3 * 128;
        }
    }

    public void Update(ThemePreferences preferences)
    {
        var skinChanged = preferences.Skin != Preferences.Skin;
        Preferences = preferences;
        Storage.Write("theme.json", preferences.ToJson());
        Recompute();
        Changed?.Invoke(skinChanged);
    }

    public void Reset() => Update(ThemePreferences.Default);

    void Recompute()
    {
        Roles = Preferences.Roles(SystemDark);
        if (HighContrast)
        {
            ApplyHighContrast();
            return;
        }
        foreach (var (name, color) in Roles.ByName()) brushes[name].Color = Color(color);
        brushes["line"].Color = Color(Roles.Line, Roles.LineOpacity);
        // Cristal lets Mica through a tinted canvas; period skins are solid (windows-app-v1 §3).
        var tinted = Roles.Skin.Backdrop() == WindowBackdrop.Mica && TransparencyEnabled;
        brushes["backdrop"].Color = Color(Roles.Canvas, tinted ? 0.82 : 1);
    }

    void ApplyHighContrast()
    {
        Windows.UI.Color Sys(UIElementType type) => ui.UIElementColor(type);
        var window = Sys(UIElementType.Window);
        var text = Sys(UIElementType.WindowText);
        var highlight = Sys(UIElementType.Highlight);
        var highlightText = Sys(UIElementType.HighlightText);
        var gray = Sys(UIElementType.GrayText);
        var link = Sys(UIElementType.Hotlight);
        foreach (var name in RoleNames) brushes[name].Color = text;
        foreach (var name in new[] { "canvas", "surface", "raised", "backdrop", "warningSoft", "accentSoft" }) brushes[name].Color = window;
        brushes["muted"].Color = gray;
        brushes["accent"].Color = highlight;
        brushes["onAccent"].Color = highlightText;
        brushes["accentText"].Color = link;
    }

    /// <summary>The XAML theme built-in controls should use with these roles.</summary>
    public ElementTheme ElementTheme => HighContrast ? ElementTheme.Default : Roles.Dark ? ElementTheme.Dark : ElementTheme.Light;

    public FontFamily DisplayFont => new(Roles.Skin.DisplayFamily());
    public FontFamily UiFont => new(Roles.Skin.UiFamily());
}

/// <summary>
/// Where a window keeps its own preferences (windows-app-v1 §9–§10). The app names its folder and its QA
/// variable once at start (`Storage.Configure("nuun", "NUUN_QA_ROOT", development)`): with the variable set,
/// everything goes under that QA root; otherwise Debug and Release use separate folders under
/// %LOCALAPPDATA%\&lt;app&gt;. An app's own data (the media catalog, nove's library) is not here: it belongs to its engine.
/// </summary>
public static class Storage
{
    static string app = "nu9ve", qaVariable = "NU9VE_QA_ROOT";
    static bool development = true;

    public static void Configure(string appFolder, string qaRootVariable, bool developmentBuild)
    {
        app = appFolder; qaVariable = qaRootVariable; development = developmentBuild;
    }

    public static string? QaRoot => Environment.GetEnvironmentVariable(qaVariable) is { Length: > 0 } qa ? qa : null;

    public static string Root => QaRoot is { } qa
        ? Path.Combine(qa, "ui")
        : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), app, development ? "Desarrollo" : "Preferencias");

    public static bool IsQa => QaRoot is not null;

    public static string? Read(string name)
    {
        try { return File.ReadAllText(Path.Combine(Root, name)); }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException) { return null; }
    }

    /// <summary>Atomic: write a temporary file, then replace.</summary>
    public static void Write(string name, string content)
    {
        try
        {
            Directory.CreateDirectory(Root);
            var path = Path.Combine(Root, name);
            File.WriteAllText(path + ".tmp", content);
            File.Move(path + ".tmp", path, overwrite: true);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException) { }
    }
}
