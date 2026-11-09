using System.Runtime.InteropServices;
using Microsoft.UI.Input;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Nu9ve.Shell;
using Nu9ve.Theme;
using Nu9ve.WinUI;
using Nu9ve.WinUI.Controls;
using Polygon = Microsoft.UI.Xaml.Shapes.Polygon;
using Rectangle = Microsoft.UI.Xaml.Shapes.Rectangle;

namespace Nuun.Setup;

/// <summary>
/// nüün's Setup window: the SD card of the icon as the window's outline (top-left corner cut, three contacts as
/// the only ornament), the app's skins, and Windows' order for its own − and × at the top right. It says what it
/// installs, where and which version before anything happens; the only action that installs is «Instalar».
/// </summary>
public sealed partial class SetupWindow : Window
{
    const double WidthDip = 600, HeightDip = 440, NotchDip = 64, CornerDip = 8;
    readonly InstallerArguments arguments;
    readonly Grid root = new();
    readonly Polygon outline = new() { StrokeThickness = 2, IsHitTestVisible = false };
    readonly Border body = new();
    readonly Grid header = new();
    CancellationTokenSource? installing;

    [LibraryImport("gdi32.dll")] private static partial nint CreatePolygonRgn([In] Point32[] points, int count, int fillMode);
    [LibraryImport("user32.dll")] private static partial int SetWindowRgn(nint hwnd, nint region, [MarshalAs(UnmanagedType.Bool)] bool redraw);
    [StructLayout(LayoutKind.Sequential)] private struct Point32 { public int X, Y; }

    public SetupWindow(InstallerArguments arguments)
    {
        this.arguments = arguments;
        Title = "Instalar nüün";
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "nuun.ico"));
        if (ThemeSkinInfo.FromId(arguments.Skin) is { } skin)
            ThemeService.Current.Update(ThemeService.Current.Preferences with { Skin = skin });
        var presenter = OverlappedPresenter.Create();
        presenter.SetBorderAndTitleBar(false, false);
        presenter.IsResizable = false;
        presenter.IsMaximizable = false;
        AppWindow.SetPresenter(presenter);

        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(96) });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        root.Children.Add(header);
        Grid.SetRow(body, 1);
        root.Children.Add(body);
        Grid.SetRowSpan(outline, 2);
        root.Children.Add(outline);
        root.KeyDown += (_, e) =>
        {
            if (e.Key != Windows.System.VirtualKey.Escape) return;
            e.Handled = true;
            if (installing is not null) installing.Cancel(); else Close();
        };
        Content = root;
        ThemeService.Current.Changed += _ => { Paint(); Welcome(); };
        root.Loaded += (_, _) =>
        {
            Size();
            Paint();
            Welcome();
        };
    }

    double Scale => root.XamlRoot?.RasterizationScale ?? 1;

    /// <summary>Size, centre, cut the outline and mark the header as the caption (drag) region.</summary>
    void Size()
    {
        var scale = Scale;
        int w = (int)(WidthDip * scale), h = (int)(HeightDip * scale);
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        AppWindow.MoveAndResize(new Windows.Graphics.RectInt32(area.X + (area.Width - w) / 2, area.Y + (area.Height - h) / 2, w, h));
        var points = WindowShapes.SdCard(w, h, (int)(NotchDip * scale), (int)(CornerDip * scale));
        var native = points.Select(p => new Point32 { X = p.X, Y = p.Y }).ToArray();
        var region = CreatePolygonRgn(native, native.Length, 2);
        _ = SetWindowRgn(WinRT.Interop.WindowNative.GetWindowHandle(this), region, true); // the window owns the region now
        outline.Points.Clear();
        foreach (var p in WindowShapes.SdCard((int)WidthDip - 1, (int)HeightDip - 1, (int)NotchDip, (int)CornerDip))
            outline.Points.Add(new Windows.Foundation.Point(p.X + 0.5, p.Y + 0.5));
        // Drag by the header, left of the window buttons (the system still handles the move).
        var caption = new Windows.Graphics.RectInt32((int)(NotchDip * scale), 0, (int)((WidthDip - NotchDip - 96) * scale), (int)(64 * scale));
        InputNonClientPointerSource.GetForWindowId(AppWindow.Id).SetRegionRects(NonClientRegionKind.Caption, [caption]);
    }

    void Paint()
    {
        var theme = ThemeService.Current;
        var roles = theme.Roles;
        root.RequestedTheme = theme.ElementTheme;
        root.Background = theme.Brush("canvas");
        outline.Stroke = roles.Skin == ThemeSkin.Glass ? ThemeService.NewBrush(roles.AccentText, 0.6) : theme.Brush("ink");
        header.Children.Clear();
        // The SD card's three contacts: the one ornament of this window (theme §7: one per context).
        var contacts = UI.Row(10);
        for (var i = 0; i < 3; i++)
            contacts.Children.Add(new Rectangle { Width = 14, Height = 22, RadiusX = Math.Min(4, roles.Skin.ControlRadius()), RadiusY = Math.Min(4, roles.Skin.ControlRadius()), Fill = ThemeService.NewBrush(roles.AccentText, 0.7) });
        contacts.Margin = new Thickness(250, 18, 0, 0);
        contacts.VerticalAlignment = VerticalAlignment.Top;
        contacts.HorizontalAlignment = HorizontalAlignment.Left;
        AutomationProperties.SetAccessibilityView(contacts, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        header.Children.Add(contacts);
        var brand = UI.Column(0, UI.Text("nüün", TextRole.Display, 34), UI.Text($"Instalador · {Version}", TextRole.Muted, 12));
        brand.Margin = new Thickness(76, 22, 0, 0);
        header.Children.Add(brand);
        // Windows order and place: minimize, then close, at the top right.
        var buttons = UI.Row(4, WindowButton("", "Minimizar", () => (AppWindow.Presenter as OverlappedPresenter)?.Minimize()),
            WindowButton("", "Cerrar instalador", () => { installing?.Cancel(); Close(); }));
        buttons.HorizontalAlignment = HorizontalAlignment.Right;
        buttons.VerticalAlignment = VerticalAlignment.Top;
        buttons.Margin = new Thickness(0, 12, 14, 0);
        header.Children.Add(buttons);
    }

    static NuButton WindowButton(string glyph, string name, Action action)
    {
        var button = new NuButton { Content = UI.Icon(glyph, 11), Padding = new Thickness(10, 6, 10, 6), MinHeight = 30 };
        ((FontIcon)button.Content).ClearValue(FontIcon.ForegroundProperty);
        AutomationProperties.SetName(button, name);
        ToolTipService.SetToolTip(button, name);
        button.Click += (_, _) => action();
        return button;
    }

    static string Version => typeof(SetupWindow).Assembly.GetName().Version is { } v ? $"{v.Major}.{v.Minor}.{v.Build}" : "0.1.0";

    UIElement Page(UIElement content, UIElement actions)
    {
        var grid = new Grid { Padding = new Thickness(36, 4, 36, 28), RowSpacing = 12 };
        grid.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        grid.Children.Add(content);
        Grid.SetRow((FrameworkElement)actions, 1);
        ((FrameworkElement)actions).HorizontalAlignment = HorizontalAlignment.Right;
        grid.Children.Add(actions);
        return grid;
    }

    // ---------- Pages ----------

    void Welcome()
    {
        if (installing is not null) return;
        var theme = ThemeService.Current;
        var facts = UI.Column(6,
            Fact("", "Qué", arguments.Demo ? $"nüün {Version} · demostración: sin paquete, no se instala nada" : $"nüün · {Path.GetFileName(arguments.Package)}"),
            Fact("", "Dónde", "Paquete MSIX para tu usuario. Se desinstala desde Configuración → Aplicaciones."),
            Fact("", "Tus datos", "%LOCALAPPDATA%\\nuun. Desinstalar no borra el catálogo ni tus preferencias."),
            Fact("", "Permisos", "No pide administrador. Confiar un certificado de desarrollo es un paso aparte."),
            Fact("", "Atribuciones", "Windows App SDK y .NET (MIT, Microsoft)."));
        var skins = UI.Row(4);
        foreach (var skin in ThemeSkinInfo.All)
            skins.Children.Add(UI.Toggle(skin.Name(), theme.Preferences.Skin == skin, () => theme.Update(theme.Preferences with { Skin = skin })));
        var content = UI.Column(14, facts, UI.Column(6, UI.Text("El instalador se viste como la app", TextRole.Label), skins));
        var install = UI.Button(arguments.Demo ? "Ver demostración" : "Instalar", () => _ = InstallAsync(), prominent: true, glyph: "");
        var actions = UI.Row(8, UI.Button("Cancelar", Close), install);
        body.Child = Page(content, actions);
        install.Loaded += (_, _) => install.Focus(FocusState.Programmatic);
    }

    static UIElement Fact(string glyph, string label, string text) =>
        UI.Columns((UI.Row(8, UI.Icon(glyph, 14, "accentText"), UI.Text(label, TextRole.Label, 12)), new GridLength(120)),
                   (UI.Text(text, TextRole.Body, 13), new GridLength(1, GridUnitType.Star)));

    async Task InstallAsync()
    {
        if (arguments.Problem(File.Exists) is { } problem) { Finish(new(InstallerExit.InvalidArguments, problem)); return; }
        installing = new CancellationTokenSource();
        var verb = UI.Text(arguments.Demo ? "Demostración: así se ve instalar" : "Instalando nüün", TextRole.Heading, 18);
        var bar = UI.Meter(0, "Progreso de la instalación", "accent");
        bar.Height = 8;
        var percent = UI.Text("0 %", TextRole.Mono);
        var note = UI.Text(arguments.Demo ? "Nada se escribe en tu equipo: no hay paquete." : "Windows instala el paquete; cancelar lo revierte.", TextRole.Muted, 12);
        body.Child = Page(UI.Column(12, verb, bar, percent, note), UI.Row(8, UI.Button("Cancelar", () => installing?.Cancel(), destructive: true)));
        var progress = new Progress<double>(value =>
        {
            bar.Value = value;
            percent.Text = $"{value:P0}";
            AutomationProperties.SetName(bar, $"Progreso de la instalación {value:P0}");
        });
        InstallResult result;
        if (arguments.Demo)
        {
            try
            {
                for (var i = 1; i <= 40; i++) { await Task.Delay(60, installing.Token); ((IProgress<double>)progress).Report(i / 40.0); }
                result = new(InstallerExit.Installed, "Demostración terminada. Con --package <nuun.msix> este botón instala de verdad.");
            }
            catch (OperationCanceledException) { result = new(InstallerExit.Cancelled, "Demostración cancelada."); }
        }
        else result = await Installer.InstallAsync(arguments.Package!, progress, installing.Token);
        installing = null;
        Finish(result);
    }

    void Finish(InstallResult result)
    {
        var ok = result.Code == InstallerExit.Installed;
        var title = result.Code switch { InstallerExit.Installed => "Listo", InstallerExit.Cancelled => "Cancelado", _ => "No se pudo instalar" };
        var icon = UI.Icon(ok ? "" : result.Code == InstallerExit.Cancelled ? "" : "", 28, ok ? "success" : result.Code == InstallerExit.Cancelled ? "muted" : "danger");
        var content = UI.Column(10, UI.Row(12, icon, UI.Text(title, TextRole.Heading, 20)), UI.Text(result.Message, TextRole.Body));
        var actions = ok && !arguments.Demo
            ? UI.Row(8, UI.Button("Cerrar", Close), UI.Button("Abrir nüün", OpenApp, prominent: true))
            : UI.Row(8, UI.Button("Volver", Welcome), UI.Button("Cerrar", Close, prominent: true));
        body.Child = Page(content, actions);
    }

    static void OpenApp()
    {
        // The packaged app is found by its Start menu entry; the family name comes with the signed package.
        _ = Windows.System.Launcher.LaunchUriAsync(new Uri("shell:AppsFolder"));
    }
}
