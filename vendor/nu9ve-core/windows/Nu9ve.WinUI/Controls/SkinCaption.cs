using Rectangle = Microsoft.UI.Xaml.Shapes.Rectangle;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Nu9ve.Theme;
using Nu9ve.WinUI;

namespace Nu9ve.WinUI.Controls;

// nu9ve-theme-v1 §5/§8 + windows-app-v1 §3 · origen: MediaMenu SkinChrome.swift SkinBarBackground
// (rayas de Macintosh, título con degradado de Windows 9x, dintel gótico, regla de Terminal).
// Divergencia Windows: es sólo el fondo de la barra. Encima va la TitleBar del sistema con sus botones a la
// derecha (minimizar, maximizar con Snap Layouts, cerrar); nada de × − + propios a la izquierda.
/// <summary>The strip behind the system title bar. Decoration: no clicks, no Narrator.</summary>
public sealed partial class SkinCaption : Grid
{
    public SkinCaption()
    {
        IsHitTestVisible = false;
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
        Loaded += (_, _) => { ThemeService.Current.Changed += OnTheme; Draw(); };
        Unloaded += (_, _) => ThemeService.Current.Changed -= OnTheme;
        SizeChanged += (_, _) => { if (ThemeService.Current.Roles.Skin == ThemeSkin.ClassicMac) Draw(); };
    }

    void OnTheme(bool _) => Draw();

    /// <summary>True when the strip is dark enough that caption text and buttons go white.</summary>
    public static bool LightGlyphs(ThemeRoles roles) => roles.Skin == ThemeSkin.Win9x || roles.Dark;

    void Draw()
    {
        Children.Clear();
        var theme = ThemeService.Current;
        var roles = theme.Roles;
        Background = null;
        if (theme.HighContrast) return;
        switch (roles.Skin)
        {
            case ThemeSkin.Win9x:
            {
                var (start, end) = roles.Win9xCaption();
                Background = new LinearGradientBrush
                {
                    StartPoint = new(0, 0.5), EndPoint = new(1, 0.5),
                    GradientStops = { new GradientStop { Color = ThemeService.Color(start), Offset = 0 }, new GradientStop { Color = ThemeService.Color(end), Offset = 1 } },
                };
                break;
            }
            case ThemeSkin.ClassicMac:
            {
                Background = new SolidColorBrush(Colors.White);
                var stripes = new StackPanel { Spacing = 1, Margin = new Thickness(0, 6, 0, 6), VerticalAlignment = VerticalAlignment.Stretch };
                var rows = Math.Max(1, (int)((ActualHeight - 12) / 2));
                for (var i = 0; i < rows; i++) stripes.Children.Add(new Rectangle { Height = 1, Fill = new SolidColorBrush(Colors.Black) });
                Children.Add(stripes);
                Children.Add(new Rectangle { Height = 1, Fill = new SolidColorBrush(Colors.Black), VerticalAlignment = VerticalAlignment.Bottom });
                break;
            }
            case ThemeSkin.Gothic:
                Background = new LinearGradientBrush
                {
                    StartPoint = new(0.5, 0), EndPoint = new(0.5, 1),
                    GradientStops = { new GradientStop { Color = ThemeService.Color(roles.Surface), Offset = 0 }, new GradientStop { Color = ThemeService.Color(roles.Canvas), Offset = 1 } },
                };
                Children.Add(new StackPanel
                {
                    VerticalAlignment = VerticalAlignment.Bottom, Spacing = 2,
                    Children =
                    {
                        new Rectangle { Height = 1, Fill = ThemeService.NewBrush(roles.Line, roles.LineOpacity) },
                        new Rectangle { Height = 1, Fill = ThemeService.NewBrush(roles.Accent, 0.5) },
                    },
                });
                break;
            case ThemeSkin.Terminal:
                Background = ThemeService.NewBrush(roles.Surface);
                Children.Add(new Rectangle { Height = 2, Fill = ThemeService.NewBrush(roles.AccentText), VerticalAlignment = VerticalAlignment.Bottom });
                break;
            default:
                // Cristal: the title bar is part of the window's surface (Mica + tinted canvas).
                break;
        }
    }
}

/// <summary>Terminal's static CRT scanlines over the window. Drawn once per size, no animation, no clicks.</summary>
public sealed partial class Scanlines : Grid
{
    public Scanlines()
    {
        IsHitTestVisible = false;
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
        SizeChanged += (_, e) => Draw(e.NewSize.Height);
    }

    void Draw(double height)
    {
        Children.Clear();
        var panel = new StackPanel { Spacing = 2 };
        var brush = new SolidColorBrush(Windows.UI.Color.FromArgb(56, 0, 0, 0));
        for (double y = 0; y < height; y += 3) panel.Children.Add(new Rectangle { Height = 1, Fill = brush });
        Children.Add(panel);
    }
}
