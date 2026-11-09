using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Nu9ve.Shell;
using Nuun.Core.Shell;
using Nu9ve.Theme;
using Nu9ve.WinUI.Controls;
using Nu9ve.WinUI;

namespace Nuun.Pages;

/// <summary>
/// Ajustes (Ctrl+,). Apariencia follows nu9ve-theme-v1 §4 in its order and names: Skin, Color, Modo, Restablecer.
/// Options nüün does not apply yet (Contraste, Textura, Tipografía, sonidos) are hidden, not renamed (§4).
/// Each control changes what is on screen right away.
/// </summary>
public static class SettingsView
{
    public static UIElement Build(MainWindow shell)
    {
        var theme = ThemeService.Current;
        var prefs = theme.Preferences;
        var column = UI.Column(12, UI.Text("Ajustes", TextRole.Display));

        // Skin cards, each painted with its own palette (theme §4 «Conviene»).
        var skins = PageScaffold.Cards(220);
        foreach (var skin in ThemeSkinInfo.All) skins.Children.Add(SkinCard(skin, prefs));
        var forced = prefs.Skin.ForcedDark();
        column.Children.Add(UI.Section("Apariencia", UI.Text("Skin", TextRole.Label), skins,
            UI.Text("Original: Cristal · Contraste: Gótico · Comunes de la suite: Macintosh, Windows 9x y Terminal. En Windows la ventana y sus botones siguen siendo del sistema.", TextRole.Muted, 12)));

        // Color: presets, hex and HSV.
        var presets = UI.Row(8);
        foreach (var preset in ThemePreset.All)
        {
            var swatch = new Border { Width = 18, Height = 18, CornerRadius = new CornerRadius(9), Background = ThemeService.NewBrush(preset.Color) };
            var toggle = UI.Toggle(preset.Name(), prefs.Seed.ToHex() == preset.Hex, () => theme.Update(theme.Preferences with { Base = preset.Hex }));
            toggle.Content = UI.Row(6, swatch, new TextBlock { Text = preset.Name() });
            presets.Children.Add(toggle);
        }
        var hex = new TextBox { Text = prefs.Seed.ToHex(), Width = 120, HorizontalAlignment = HorizontalAlignment.Left };
        AutomationProperties.SetName(hex, "Color base en hexadecimal");
        hex.LostFocus += (_, _) =>
        {
            if (ThemeColor.FromHex(hex.Text) is { } color) theme.Update(theme.Preferences with { Base = color.ToHex() });
            else hex.Text = theme.Preferences.Seed.ToHex();
        };
        var (h, s, v) = prefs.Seed.Hsv;
        var hue = Slider("Tono", h * 360, 360);
        var saturation = Slider("Saturación", s * 100, 100);
        var value = Slider("Brillo", v * 100, 100);
        void Commit()
        {
            var color = ThemeColor.FromHsv(hue.Value / 360 % 1, saturation.Value / 100, value.Value / 100);
            hex.Text = color.ToHex();
            theme.Update(theme.Preferences with { Base = color.ToHex() });
        }
        foreach (var slider in new[] { hue, saturation, value }) slider.PointerCaptureLost += (_, _) => Commit();
        foreach (var slider in new[] { hue, saturation, value }) slider.KeyUp += (_, _) => Commit();
        column.Children.Add(UI.Surface(SurfaceKind.Panel, UI.Column(10,
            UI.Text("Color", TextRole.Label), presets,
            UI.Row(12, UI.Text("Hex", TextRole.Muted, 12), hex),
            Labeled("Tono", hue), Labeled("Saturación", saturation), Labeled("Brillo", value))));

        // Mode.
        var modes = UI.Row(4);
        foreach (var (mode, label) in new[] { (ThemeMode.Light, "Claro"), (ThemeMode.Dark, "Oscuro"), (ThemeMode.System, "Sistema") })
        {
            var toggle = UI.Toggle(label, prefs.Mode == mode, () => theme.Update(theme.Preferences with { Mode = mode }));
            toggle.IsEnabled = forced is null;
            modes.Children.Add(toggle);
        }
        column.Children.Add(UI.Surface(SurfaceKind.Panel, UI.Column(8,
            UI.Text("Modo", TextRole.Label), modes,
            forced is { } dark
                ? UI.Text($"{prefs.Skin.Name()} es {(dark ? "oscuro" : "claro")} por su época; tu preferencia ({ModeName(prefs.Mode)}) vuelve al elegir otro skin.", TextRole.Muted, 12)
                : UI.Text("«Sistema» sigue el modo de Windows sin cambiar el color de nüün.", TextRole.Muted, 12))));

        column.Children.Add(UI.Row(8,
            UI.Button("Restablecer apariencia", theme.Reset, glyph: ""),
            UI.Text(theme.HighContrast ? "Alto contraste de Windows activo: mandan los colores del sistema." : "", TextRole.Muted, 12)));

        column.Children.Add(UI.Section("Teclado",
            UI.Text("Todo se puede hacer con el teclado. Alt muestra las teclas de acceso del sistema; Ctrl+/ abre todos los atajos.", TextRole.Muted, 13),
            UI.Button("Ver atajos", shell.ShowShortcuts, glyph: "", shortcut: "app.shortcuts")));

        var engine = shell.State.Engine switch
        {
            Nuun.Core.Engine.MethodByMethodEngine mixed =>
                $"Motor de Rust (engine\\nuun-engine.exe) para lo ya portado: {(mixed.AnsweredByPrimary.Count == 0 ? "nada todavía" : string.Join(", ", mixed.AnsweredByPrimary.Order()))}. El resto sigue en maqueta, con datos ficticios.",
            { IsFixture: true } => "Maqueta: datos ficticios, ningún disco se lee ni se escribe. Con NUUN_PYTHON y NUUN_ENGINE (ruta a nu9ve.py) la ventana lee snapshot y recipe del motor de Python; con NUUN_ENGINE_BIN, del motor de Rust.",
            _ => "Motor: media rpc-serve. Planear y aplicar siguen pendientes en Windows (adaptador de volúmenes).",
        };
        column.Children.Add(UI.Section("Motor", UI.Text(engine, TextRole.Muted, 13),
            UI.Text($"Preferencias de la ventana: {Storage.Root}", TextRole.Mono)));
        column.Children.Add(UI.Section("Acerca de", UI.Button("Acerca de nüün", () => shell.Navigate("about"), glyph: "")));
        return PageScaffold.Build(column, null, 760);
    }

    static string ModeName(ThemeMode mode) => mode switch { ThemeMode.Light => "Claro", ThemeMode.Dark => "Oscuro", _ => "Sistema" };

    static Slider Slider(string name, double value, double max)
    {
        var slider = new Slider { Minimum = 0, Maximum = max, Value = value, StepFrequency = 1, Width = 260, HorizontalAlignment = HorizontalAlignment.Left };
        AutomationProperties.SetName(slider, name);
        return slider;
    }

    static UIElement Labeled(string label, UIElement control) => UI.Row(12, new TextBlock { Text = label, Width = 80, Foreground = ThemeService.Current.Brush("muted") }, control);

    /// <summary>A card that previews the skin with its own roles (not the current ones).</summary>
    static UIElement SkinCard(ThemeSkin skin, ThemePreferences prefs)
    {
        var theme = ThemeService.Current;
        var roles = ThemeRoles.From(skin, theme.Roles.Dark, prefs.Seed);
        var selected = prefs.Skin == skin;
        var strip = new Border
        {
            Height = 18,
            Background = skin == ThemeSkin.Win9x
                ? new LinearGradientBrush { StartPoint = new(0, 0.5), EndPoint = new(1, 0.5), GradientStops = { new GradientStop { Color = ThemeService.Color(roles.Win9xCaption().Start) }, new GradientStop { Color = ThemeService.Color(roles.Win9xCaption().End), Offset = 1 } } }
                : ThemeService.NewBrush(roles.Surface),
            BorderBrush = ThemeService.NewBrush(roles.Line, roles.LineOpacity), BorderThickness = new Thickness(0, 0, 0, 1),
        };
        var preview = new Border
        {
            Background = ThemeService.NewBrush(roles.Canvas), CornerRadius = new CornerRadius(skin.PanelRadius()),
            BorderBrush = ThemeService.NewBrush(roles.Line, roles.LineOpacity), BorderThickness = new Thickness(skin == ThemeSkin.Win9x ? 2 : 1),
            Child = new StackPanel
            {
                Children =
                {
                    strip,
                    new StackPanel
                    {
                        Padding = new Thickness(10, 8, 10, 10), Spacing = 4,
                        Children =
                        {
                            new TextBlock { Text = skin.Name(), FontFamily = new FontFamily(skin.DisplayFamily()), FontSize = 16, Foreground = ThemeService.NewBrush(roles.Ink) },
                            new TextBlock { Text = skin.Summary(), FontSize = 11, TextWrapping = TextWrapping.Wrap, Foreground = ThemeService.NewBrush(roles.Muted), Height = 44 },
                            new Border { Height = 18, Width = 70, HorizontalAlignment = HorizontalAlignment.Left, CornerRadius = new CornerRadius(Math.Min(9, skin.ControlRadius())), Background = ThemeService.NewBrush(roles.Accent) },
                        },
                    },
                },
            },
        };
        var toggle = new NuToggle { Card = true, Content = preview, IsChecked = selected, Padding = new Thickness(4), Margin = new Thickness(0, 0, 10, 10), HorizontalContentAlignment = HorizontalAlignment.Stretch };
        AutomationProperties.SetName(toggle, $"Skin {skin.Name()}: {skin.Summary()}");
        toggle.Click += (_, _) => theme.Update(theme.Preferences with { Skin = skin });
        return toggle;
    }
}
