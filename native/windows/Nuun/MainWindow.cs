using System.Reflection;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Nu9ve.Shell;
using Nuun.Core.Shell;
using Nu9ve.Theme;
using Nu9ve.WinUI.Controls;
using Nuun.Pages;
using Nu9ve.WinUI;
using Windows.System;

namespace Nuun;

/// <summary>
/// One window, three presentations (compact, selection, full) of the same <see cref="AppState"/>.
/// windows-app-v1 §1: the system's TitleBar (drag, Snap Layouts, caption buttons on the right); nüün's bar
/// — wordmark, navigation, disks and actions — lives inside it, or in a second row under a period caption
/// (Macintosh, Windows 9x), the same rule as the Mac app's SkinWindowBar.
/// </summary>
public sealed partial class MainWindow : Window
{
    public AppState State { get; }
    readonly Grid root = new();
    readonly SkinCaption caption = new();
    readonly TitleBar titleBar = new() { IsBackButtonVisible = false, IsPaneToggleButtonVisible = false };
    readonly Grid toolbarRow = new() { Padding = new Thickness(8, 0, 8, 0) };
    readonly Border toolbarRule = new() { Height = 2, VerticalAlignment = VerticalAlignment.Bottom, IsHitTestVisible = false };
    readonly StackPanel brand = new() { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(12, 0, 8, 0) };
    // Macintosh: the title sits in a white box over the pinstripes (MediaMenu SkinWindowBar).
    readonly Border brandBox = new() { VerticalAlignment = VerticalAlignment.Center };
    readonly StackPanel navigation = new() { Orientation = Orientation.Horizontal, Spacing = 4, VerticalAlignment = VerticalAlignment.Center };
    readonly NuSurface navGroup = new() { Kind = SurfaceKind.Group, Padding = new Thickness(3), VerticalAlignment = VerticalAlignment.Center };
    readonly StackPanel actions = new() { Orientation = Orientation.Horizontal, Spacing = 6, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 8, 0) };
    readonly StackPanel disks = new() { Orientation = Orientation.Horizontal, Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
    readonly Border body = new();
    readonly InfoBar errorBar = new() { Severity = InfoBarSeverity.Error, IsOpen = false, Title = "Atención", Margin = new Thickness(16, 8, 16, 0) };
    readonly Scanlines scanlines = new() { Visibility = Visibility.Collapsed };
    readonly TextBlock wordmark = new() { Text = "nüün", FontSize = 15, VerticalAlignment = VerticalAlignment.Center };
    readonly TextBlock subtitle = new() { FontSize = 12, VerticalAlignment = VerticalAlignment.Center };
    string page = "home";
    IngestView? ingest;

    public static readonly string[] Pages = ["home", "ingest", "flows", "library"];

    public MainWindow()
    {
        State = new AppState(DispatcherQueue);
        Title = "nüün";
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "nuun.ico"));
        ExtendsContentIntoTitleBar = true;
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.PreferredMinimumWidth = 400;
            presenter.PreferredMinimumHeight = 560;
        }

        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        root.Background = ThemeService.Current.Brush("backdrop");

        var titleArea = new Grid();
        titleArea.Children.Add(caption);
        titleArea.Children.Add(titleBar);
        root.Children.Add(titleArea);
        toolbarRow.Children.Add(toolbarRule);
        Grid.SetRow(toolbarRow, 1);
        root.Children.Add(toolbarRow);
        Grid.SetRow(errorBar, 2);
        root.Children.Add(errorBar);
        Grid.SetRow(body, 3);
        root.Children.Add(body);
        Grid.SetRowSpan(scanlines, 4);
        root.Children.Add(scanlines);
        errorBar.CloseButtonClick += (_, _) => State.ClearError();

        brand.Children.Add(wordmark);
        brand.Children.Add(subtitle);
        navGroup.Child = navigation;
        brandBox.Child = brand;
        AutomationProperties.SetName(wordmark, "nüün");
        AutomationProperties.SetHeadingLevel(wordmark, AutomationHeadingLevel.Level1);

        Content = root;
        SetTitleBar(titleBar);
        InstallAccelerators();

        ThemeService.Current.Follow(DispatcherQueue);
        ThemeService.Current.Changed += skinChanged => ApplyTheme(skinChanged);
        root.Loaded += (_, _) =>
        {
            ApplyTheme(true);
            SetMode(Program.RequestedCompact ? WindowMode.Compact : WindowMode.Full, initial: true);
            Navigate(Program.RequestedPage ?? "home");
        };
        root.SizeChanged += (_, e) => Fit(e.NewSize.Width);
        State.PropertyChanged += (_, e) => OnState(e.PropertyName);
        Activated += (_, e) => ingest?.SetActive(e.WindowActivationState != WindowActivationState.Deactivated);
        _ = State.StartAsync();
    }

    public void BringToFront()
    {
        if (AppWindow.Presenter is OverlappedPresenter { State: OverlappedPresenterState.Minimized } presenter) presenter.Restore();
        Activate();
    }

    // ---------- Theme and bar ----------

    bool TwoRows => ThemeService.Current.Roles.Skin is ThemeSkin.ClassicMac or ThemeSkin.Win9x;

    void ApplyTheme(bool skinChanged)
    {
        var theme = ThemeService.Current;
        WindowChrome.Apply(this, root);
        var roles = theme.Roles;
        scanlines.Visibility = roles.Skin == ThemeSkin.Terminal && !theme.HighContrast ? Visibility.Visible : Visibility.Collapsed;
        wordmark.FontFamily = theme.DisplayFont;
        wordmark.FontWeight = Microsoft.UI.Text.FontWeights.SemiBold;
        var light = roles.Skin == ThemeSkin.Win9x && !theme.HighContrast;
        wordmark.Foreground = light ? new SolidColorBrush(Microsoft.UI.Colors.White) : theme.Brush("ink");
        subtitle.Foreground = light ? new SolidColorBrush(Windows.UI.Color.FromArgb(220, 255, 255, 255)) : theme.Brush("muted");
        subtitle.FontFamily = theme.UiFont;
        var mac = roles.Skin == ThemeSkin.ClassicMac && !theme.HighContrast;
        brandBox.Background = mac ? new SolidColorBrush(Microsoft.UI.Colors.White) : null;
        brandBox.Padding = mac ? new Thickness(6, 0, 6, 0) : new Thickness(0);
        brand.Margin = mac ? new Thickness(0) : new Thickness(12, 0, 8, 0);
        brandBox.Margin = mac ? new Thickness(12, 0, 8, 0) : new Thickness(0);
        toolbarRule.Background = roles.Skin == ThemeSkin.Win9x ? ThemeService.NewBrush(new ThemeColor(128 / 255.0, 128 / 255.0, 128 / 255.0)) : theme.Brush("ink");
        if (skinChanged)
        {
            Arrange();
            BuildBar();
            Navigate(page);
        }
    }

    /// <summary>Single row in the TitleBar, or caption + toolbar row for period skins (MediaMenu SkinWindowBar).</summary>
    void Arrange()
    {
        Detach(brandBox); Detach(navGroup); Detach(actions);
        titleBar.LeftHeader = null; titleBar.Content = null; titleBar.RightHeader = null;
        var grid = toolbarRow.Children.OfType<Grid>().FirstOrDefault();
        if (grid is not null) toolbarRow.Children.Remove(grid);
        if (TwoRows)
        {
            AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Standard;
            titleBar.LeftHeader = brandBox;
            var row = UI.Columns((navGroup, new GridLength(1, GridUnitType.Star)), (actions, GridLength.Auto));
            navGroup.HorizontalAlignment = HorizontalAlignment.Left;
            row.Height = ThemeService.Current.Roles.Skin == ThemeSkin.Win9x ? 40 : 38;
            toolbarRow.Children.Insert(0, row);
            toolbarRow.Visibility = Visibility.Visible;
        }
        else
        {
            AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
            navGroup.HorizontalAlignment = HorizontalAlignment.Center;
            titleBar.LeftHeader = brandBox;
            titleBar.Content = navGroup;
            titleBar.RightHeader = actions;
            toolbarRow.Visibility = Visibility.Collapsed;
        }
        static void Detach(FrameworkElement element)
        {
            switch (element.Parent)
            {
                case Panel panel: panel.Children.Remove(element); break;
                case TitleBar bar:
                    if (bar.LeftHeader == element) bar.LeftHeader = null;
                    if (bar.Content == element) bar.Content = null;
                    if (bar.RightHeader == element) bar.RightHeader = null;
                    break;
            }
        }
    }

    void BuildBar()
    {
        navigation.Children.Clear();
        var items = new (string Id, string Label, string Glyph, string Shortcut, string AccessKey)[]
        {
            ("home", "Inicio", "", "nav.home", "I"),
            ("ingest", "Ingreso", "", "nav.ingest", "N"),
            ("flows", "Flujos", "", "nav.flows", "F"),
            ("library", "Biblioteca", "", "nav.library", "B"),
        };
        foreach (var item in items)
        {
            var toggle = UI.Toggle(item.Label, page == item.Id, () => Navigate(item.Id), item.Glyph, item.Shortcut);
            toggle.AccessKey = item.AccessKey;
            toggle.Tag = item.Id;
            navigation.Children.Add(toggle);
        }
        actions.Children.Clear();
        actions.Children.Add(disks);
        BuildDisks();
        if (State.Engine.IsFixture)
        {
            var badge = UI.Badge("Maqueta", SurfaceKind.Well);
            ToolTipService.SetToolTip(badge, "Datos ficticios: no se lee ni se escribe ningún disco");
            AutomationProperties.SetName(badge, "Maqueta: datos ficticios");
            actions.Children.Add(badge);
        }
        actions.Children.Add(IconButton("", "Atajos de teclado", "app.shortcuts", ShowShortcuts));
        actions.Children.Add(IconButton("", "Ajustes", "app.settings", () => Navigate("settings")));
        actions.Children.Add(IconButton(State.Mode == WindowMode.Compact ? "" : "", State.Mode == WindowMode.Compact ? "Ventana completa" : "Ventana compacta", "app.mode", ToggleMode));
        Fit(root.ActualWidth);
    }

    void BuildDisks()
    {
        disks.Children.Clear();
        foreach (var volume in State.Snapshot.Volumes.Where(v => v.Online))
        {
            var chip = UI.Button($"{volume.Name} · {Presentation.Bytes(volume.Free)}", () => Navigate("library"), glyph: "");
            chip.Padding = new Thickness(8, 2, 8, 2);
            chip.MinHeight = 26;
            AutomationProperties.SetName(chip, $"{volume.Name}, {Presentation.Bytes(volume.Free)} libres. Abre Discos");
            disks.Children.Add(chip);
        }
    }

    static NuButton IconButton(string glyph, string name, string shortcut, Action action)
    {
        var button = new NuButton { Content = UI.Icon(glyph, 15), Padding = new Thickness(8, 4, 8, 4), MinHeight = 30 };
        ((FontIcon)button.Content).ClearValue(FontIcon.ForegroundProperty);
        AutomationProperties.SetName(button, name);
        UI.Describe(button, name, shortcut);
        button.Click += (_, _) => action();
        return button;
    }

    /// <summary>windows-app-v1 §1: the bar gives way step by step with a pure, tested rule (TitleBarFit).</summary>
    void Fit(double width)
    {
        if (width <= 0) return;
        var fit = TitleBarFit.For(width);
        disks.Visibility = fit.Disks ? Visibility.Visible : Visibility.Collapsed;
        subtitle.Visibility = fit.Subtitle ? Visibility.Visible : Visibility.Collapsed;
        wordmark.Visibility = fit.Wordmark ? Visibility.Visible : Visibility.Collapsed;
        foreach (var toggle in navigation.Children.OfType<NuToggle>())
            if (toggle.Content is StackPanel { Children.Count: 2 } row) row.Children[1].Visibility = fit.NavigationLabels ? Visibility.Visible : Visibility.Collapsed;
    }

    // ---------- Navigation ----------

    public void Navigate(string target)
    {
        page = target;
        foreach (var toggle in navigation.Children.OfType<NuToggle>()) toggle.IsChecked = (string?)toggle.Tag == page;
        subtitle.Text = page switch
        {
            "ingest" => "Ingreso", "flows" => "Flujos", "library" => "Biblioteca", "settings" => "Ajustes", "about" => "Acerca de", _ => "Inicio",
        };
        ingest = null;
        UIElement view = page switch
        {
            "ingest" => (ingest = new IngestView(State, this)).Root,
            "flows" => FlowsView.Build(State),
            "library" => LibraryView.Build(State, this),
            "settings" => SettingsView.Build(this),
            "about" => AboutView.Build(),
            _ => HomeView.Build(State, this),
        };
        body.Child = view;
        Fit(root.ActualWidth);
    }

    void OnState(string? property)
    {
        switch (property)
        {
            case nameof(AppState.Error):
                errorBar.Message = State.Error ?? "";
                errorBar.IsOpen = State.Error is not null;
                break;
            case nameof(AppState.Snapshot):
                BuildBar();
                if (page is "home" or "library" or "flows" or "ingest") Navigate(page);
                break;
            case nameof(AppState.Progress):
                ingest?.UpdateProgress();
                break;
            case nameof(AppState.Step) or nameof(AppState.Source) or nameof(AppState.Busy):
                if (page == "ingest") ingest?.Rebuild(); else if (property == nameof(AppState.Step)) Navigate("ingest");
                break;
        }
    }

    // ---------- Modes ----------

    public void ToggleMode() => SetMode(WindowLayout.Toggle(State.Mode));

    public void SetMode(WindowMode mode, bool initial = false)
    {
        State.Mode = mode;
        var (width, height) = WindowLayout.Size(mode);
        var scale = root.XamlRoot?.RasterizationScale ?? 1;
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Nearest).WorkArea;
        var frame = WindowLayout.Fit((AppWindow.Position.X, AppWindow.Position.Y, (int)(width * scale), (int)(height * scale)), (area.X, area.Y, area.Width, area.Height));
        if (initial)
            frame = WindowLayout.Fit((area.X + (area.Width - frame.Width) / 2, area.Y + (area.Height - frame.Height) / 2, frame.Width, frame.Height), (area.X, area.Y, area.Width, area.Height));
        AppWindow.MoveAndResize(new Windows.Graphics.RectInt32(frame.X, frame.Y, frame.Width, frame.Height));
        if (AppWindow.Presenter is OverlappedPresenter presenter) presenter.IsAlwaysOnTop = WindowLayout.AlwaysOnTop(mode);
        if (!initial) BuildBar();
    }

    // ---------- Keyboard (nu9ve-interaction-v1 §1: every accelerator comes from the catalog) ----------

    void InstallAccelerators()
    {
        foreach (var shortcut in ShortcutCatalog.All)
        {
            if (shortcut.Keys is not { } chord || Key(chord.Key) is not { } key) continue;
            var accelerator = new KeyboardAccelerator { Key = key, Modifiers = Mods(chord.Modifiers) };
            var id = shortcut.Id;
            accelerator.Invoked += (_, e) => e.Handled = Run(id);
            root.KeyboardAccelerators.Add(accelerator);
        }
        // Accelerators with no visible element: hide their default tooltip text.
        root.KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden;
    }

    bool Run(string id)
    {
        switch (id)
        {
            case "nav.home": Navigate("home"); return true;
            case "nav.ingest": Navigate("ingest"); return true;
            case "nav.flows": Navigate("flows"); return true;
            case "nav.library": Navigate("library"); return true;
            case "app.settings": Navigate("settings"); return true;
            case "app.shortcuts": ShowShortcuts(); return true;
            case "app.help": _ = Launcher.LaunchUriAsync(new Uri("https://github.com/nu9ve/n-creator-utils/blob/master/docs/operations.md")); return true;
            case "app.mode": ToggleMode(); return true;
            case "app.fullscreen":
                AppWindow.SetPresenter(AppWindow.Presenter.Kind == AppWindowPresenterKind.FullScreen ? AppWindowPresenterKind.Overlapped : AppWindowPresenterKind.FullScreen);
                return true;
            case "ingest.new": State.NewIngest(); Navigate("ingest"); return true;
            default:
                // View shortcuts: only while their view is on screen; otherwise «no disponible» (interaction §7).
                if (page == "ingest" && ingest is not null) return ingest.Run(id);
                State.Report($"«{ShortcutCatalog.Get(id).Title}» no está disponible en esta vista");
                return true;
        }
    }

    static VirtualKey? Key(string name) => name switch
    {
        "Comma" => (VirtualKey)188, "Slash" => (VirtualKey)191, "Backslash" => (VirtualKey)220,
        "BracketLeft" => (VirtualKey)219, "BracketRight" => (VirtualKey)221, "Enter" => VirtualKey.Enter,
        { Length: 1 } digit when char.IsDigit(digit[0]) => VirtualKey.Number0 + (digit[0] - '0'),
        { Length: 1 } letter when char.IsLetter(letter[0]) => Enum.Parse<VirtualKey>(letter),
        _ when Enum.TryParse<VirtualKey>(name, out var parsed) => parsed,
        _ => null,
    };

    static VirtualKeyModifiers Mods(Modifiers modifiers) =>
        (modifiers.HasFlag(Modifiers.Ctrl) ? VirtualKeyModifiers.Control : 0)
        | (modifiers.HasFlag(Modifiers.Shift) ? VirtualKeyModifiers.Shift : 0)
        | (modifiers.HasFlag(Modifiers.Alt) ? VirtualKeyModifiers.Menu : 0);

    /// <summary>Ctrl+/: the shortcut panel by task (interaction §5), themed like every window.</summary>
    public async void ShowShortcuts()
    {
        var list = UI.Column(4);
        foreach (var group in ShortcutCatalog.All.GroupBy(s => s.Group))
        {
            list.Children.Add(UI.Text(group.Key, TextRole.Label));
            foreach (var shortcut in group)
            {
                var keys = shortcut.Keys?.Label ?? "—";
                list.Children.Add(UI.Columns(
                    (UI.Keycap(keys), new GridLength(96)),
                    (UI.Column(0, UI.Text(shortcut.Title), UI.Text(shortcut.Help, TextRole.Muted, 12)), new GridLength(1, GridUnitType.Star))));
            }
        }
        var dialog = new ContentDialog
        {
            XamlRoot = root.XamlRoot, Title = "Atajos de teclado", CloseButtonText = "Cerrar",
            Content = new ScrollViewer { Content = list, MaxHeight = 460 },
            RequestedTheme = ThemeService.Current.ElementTheme,
            Background = ThemeService.Current.Brush("surface"), Foreground = ThemeService.Current.Brush("ink"),
        };
        try { await dialog.ShowAsync(); } catch (InvalidOperationException) { }
    }

    public static string Version =>
        typeof(MainWindow).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion ?? "0.1.0";
}
