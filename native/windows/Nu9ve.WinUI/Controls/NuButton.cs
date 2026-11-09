using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using Nu9ve.Theme;
using Nu9ve.WinUI;

namespace Nu9ve.WinUI.Controls;

/// <summary>
/// Shared look of skinned buttons: maps the button's state to a <see cref="SurfaceKind"/> and applies the
/// spec's text color, weight and press offset. Behavior (keyboard, Narrator, access keys) stays Button's.
/// Bound from OnApplyTemplate: Loaded can arrive before the template exists inside a custom panel.
/// </summary>
sealed class SkinButtonChrome
{
    readonly ButtonBase button;
    readonly Func<SurfaceKind> kind;
    NuSurface? surface;
    ContentPresenter? presenter;

    public SkinButtonChrome(ButtonBase button, Func<SurfaceKind> kind)
    {
        this.button = button;
        this.kind = kind;
        void OnTheme(bool _) => Apply();
        button.Loaded += (_, _) => { ThemeService.Current.Changed += OnTheme; Apply(); };
        button.Unloaded += (_, _) => ThemeService.Current.Changed -= OnTheme;
        button.RegisterPropertyChangedCallback(ButtonBase.IsPressedProperty, (_, _) => Apply());
        button.RegisterPropertyChangedCallback(ButtonBase.IsPointerOverProperty, (_, _) => Apply());
        button.IsEnabledChanged += (_, _) => Apply();
        if (button is ToggleButton toggle) toggle.RegisterPropertyChangedCallback(ToggleButton.IsCheckedProperty, (_, _) => Apply());
    }

    public void Bind(NuSurface? templateSurface)
    {
        surface = templateSurface;
        presenter = surface?.Child as ContentPresenter;
        Apply();
    }

    public void Apply()
    {
        var theme = ThemeService.Current;
        var current = kind();
        var spec = SurfaceChrome.For(theme.Roles, current);
        // Text and type apply even before the template, so a button never shows the system look.
        button.Foreground = theme.HighContrast ? theme.Brush("ink") : ThemeService.NewBrush(spec.Foreground);
        button.FontWeight = spec.Strong ? FontWeights.SemiBold : FontWeights.Normal;
        button.FontFamily = theme.UiFont;
        button.Opacity = button.IsEnabled ? 1 : 0.45;
        button.FocusVisualPrimaryBrush = theme.Brush("ink");
        button.FocusVisualSecondaryBrush = theme.Brush("canvas");
        if (surface is null) return;
        surface.Kind = current;
        surface.Hovered = button.IsPointerOver && !button.IsPressed;
        if (presenter is not null)
            presenter.RenderTransform = spec.PressOffset > 0 ? new TranslateTransform { X = spec.PressOffset, Y = spec.PressOffset } : null;
    }
}

/// <summary>A command. One <see cref="Prominent"/> per view (nu9ve-theme-v1 §8); <see cref="Destructive"/> uses danger.</summary>
public sealed partial class NuButton : Button
{
    readonly SkinButtonChrome chrome;
    public bool Prominent { get; set; }
    public bool Destructive { get; set; }

    public NuButton()
    {
        DefaultStyleKey = typeof(NuButton);
        chrome = new SkinButtonChrome(this, () => Prominent
            ? (IsPressed ? SurfaceKind.PrimaryPressed : SurfaceKind.Primary)
            : Destructive ? SurfaceKind.Danger
            : IsPressed ? SurfaceKind.ControlPressed : SurfaceKind.Control);
    }

    protected override void OnApplyTemplate()
    {
        base.OnApplyTemplate();
        chrome.Bind(GetTemplateChild("Surface") as NuSurface);
    }
}

/// <summary>A pill that stays selected: navigation, steps, filters and choices.</summary>
public sealed partial class NuToggle : ToggleButton
{
    readonly SkinButtonChrome chrome;
    /// <summary>A whole card that can be chosen (a source, a skin) instead of a pill.</summary>
    public bool Card { get; set; }

    public NuToggle()
    {
        DefaultStyleKey = typeof(NuToggle);
        chrome = new SkinButtonChrome(this, () => Card
            ? (IsChecked == true ? SurfaceKind.CardSelected : SurfaceKind.Card)
            : IsChecked == true ? SurfaceKind.PillSelected : SurfaceKind.Pill);
    }

    protected override void OnApplyTemplate()
    {
        base.OnApplyTemplate();
        chrome.Bind(GetTemplateChild("Surface") as NuSurface);
    }
}
