using System.Runtime.InteropServices;
using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Nu9ve.Theme;
using Nu9ve.WinUI.Controls;
using Nu9ve.WinUI;

namespace Nu9ve.WinUI;

/// <summary>
/// windows-app-v1 §1–§3 for a window: system caption buttons tinted to the theme (released in high contrast),
/// Mica for Cristal and a solid canvas for period skins, square corners through DWM for period skins.
/// The window keeps the system's caption, drag, Snap Layouts and buttons on the right.
/// </summary>
public static partial class WindowChrome
{
    const int DwmwaWindowCornerPreference = 33;
    const int DwmwcpDefault = 0, DwmwcpDoNotRound = 1;

    [LibraryImport("dwmapi.dll")]
    private static partial int DwmSetWindowAttribute(nint hwnd, int attribute, ref int value, int size);

    public static nint Handle(Window window) => WinRT.Interop.WindowNative.GetWindowHandle(window);

    public static void Apply(Window window, FrameworkElement root)
    {
        var theme = ThemeService.Current;
        var roles = theme.Roles;
        root.RequestedTheme = theme.ElementTheme;
        // Mica only where Cristal asks for it and transparency effects are on; otherwise a solid canvas.
        var mica = roles.Skin.Backdrop() == WindowBackdrop.Mica && theme.TransparencyEnabled && !theme.HighContrast;
        if (mica && window.SystemBackdrop is not MicaBackdrop) window.SystemBackdrop = new MicaBackdrop();
        else if (!mica && window.SystemBackdrop is not null) window.SystemBackdrop = null;
        var corner = roles.Skin.RoundedWindow() ? DwmwcpDefault : DwmwcpDoNotRound;
        _ = DwmSetWindowAttribute(Handle(window), DwmwaWindowCornerPreference, ref corner, sizeof(int));
        TintCaptionButtons(window.AppWindow.TitleBar, roles, theme.HighContrast);
    }

    /// <summary>
    /// The caption buttons are drawn by Windows and do not follow the content on their own (origin: nove
    /// Settings.ApplyTheme). On Windows 9x's gradient and on dark skins the glyphs go white.
    /// </summary>
    public static void TintCaptionButtons(AppWindowTitleBar bar, ThemeRoles roles, bool highContrast)
    {
        if (highContrast)
        {
            bar.ButtonForegroundColor = bar.ButtonHoverForegroundColor = bar.ButtonInactiveForegroundColor = bar.ButtonPressedForegroundColor = null;
            bar.ButtonBackgroundColor = bar.ButtonInactiveBackgroundColor = bar.ButtonHoverBackgroundColor = bar.ButtonPressedBackgroundColor = null;
            return;
        }
        var light = SkinCaption.LightGlyphs(roles);
        var ink = light ? Colors.White : ThemeService.Color(roles.Ink);
        bar.ButtonForegroundColor = ink;
        bar.ButtonHoverForegroundColor = ink;
        bar.ButtonPressedForegroundColor = ink;
        bar.ButtonInactiveForegroundColor = light ? Windows.UI.Color.FromArgb(170, 255, 255, 255) : ThemeService.Color(roles.Muted);
        var paper = roles.Skin == ThemeSkin.ClassicMac;
        bar.ButtonBackgroundColor = paper ? Colors.White : Colors.Transparent;
        bar.ButtonInactiveBackgroundColor = paper ? Colors.White : Colors.Transparent;
        bar.ButtonHoverBackgroundColor = light ? Windows.UI.Color.FromArgb(48, 255, 255, 255) : Windows.UI.Color.FromArgb(24, 0, 0, 0);
        bar.ButtonPressedBackgroundColor = light ? Windows.UI.Color.FromArgb(80, 255, 255, 255) : Windows.UI.Color.FromArgb(48, 0, 0, 0);
    }
}
