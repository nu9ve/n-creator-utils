using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Markup;
using Nu9ve.Theme;
using Nu9ve.WinUI;
using Windows.Foundation;

namespace Nu9ve.WinUI.Controls;

/// <summary>
/// A surface drawn by the active skin: fill, radius and edges from <see cref="SurfaceChrome"/> (pure, tested).
/// Views say what the surface is (<see cref="Kind"/>), never which skin is on (nu9ve-theme-v1 §1). The edges are
/// decorative Borders: no hit testing and outside the UI Automation tree, like the Mac app's bevels.
/// </summary>
[ContentProperty(Name = nameof(Child))]
public sealed partial class NuSurface : Panel
{
    readonly Border fill = Decor();
    readonly List<Border> rings = [];
    UIElement? child;
    SurfaceSpec spec = SurfaceChrome.For(ThemeService.Current.Roles, SurfaceKind.Panel);

    public static readonly DependencyProperty KindProperty = DependencyProperty.Register(nameof(Kind), typeof(SurfaceKind),
        typeof(NuSurface), new PropertyMetadata(SurfaceKind.Panel, (d, _) => ((NuSurface)d).Refresh()));
    public static readonly DependencyProperty PaddingProperty = DependencyProperty.Register(nameof(Padding), typeof(Thickness),
        typeof(NuSurface), new PropertyMetadata(new Thickness(0), (d, _) => ((NuSurface)d).InvalidateMeasure()));
    public static readonly DependencyProperty HoveredProperty = DependencyProperty.Register(nameof(Hovered), typeof(bool),
        typeof(NuSurface), new PropertyMetadata(false, (d, _) => ((NuSurface)d).Refresh()));

    public SurfaceKind Kind { get => (SurfaceKind)GetValue(KindProperty); set => SetValue(KindProperty, value); }
    public Thickness Padding { get => (Thickness)GetValue(PaddingProperty); set => SetValue(PaddingProperty, value); }
    /// <summary>Pointer over: the fill leans 6 % toward ink (an invisible pill shows its surface).</summary>
    public bool Hovered { get => (bool)GetValue(HoveredProperty); set => SetValue(HoveredProperty, value); }

    public SurfaceSpec Spec => spec;

    public UIElement? Child
    {
        get => child;
        set
        {
            if (child is not null) Children.Remove(child);
            child = value;
            if (child is not null) Children.Add(child);
            InvalidateMeasure();
        }
    }

    public NuSurface()
    {
        Children.Add(fill);
        Refresh();
        Loaded += (_, _) => { ThemeService.Current.Changed += OnTheme; Refresh(); };
        Unloaded += (_, _) => ThemeService.Current.Changed -= OnTheme;
    }

    protected override AutomationPeer OnCreateAutomationPeer() => new FrameworkElementAutomationPeer(this);

    static Border Decor()
    {
        var border = new Border { IsHitTestVisible = false };
        AutomationProperties.SetAccessibilityView(border, AccessibilityView.Raw);
        return border;
    }

    void OnTheme(bool _) => Refresh();

    double Inset => spec.Edges.Sum(e => e.Width);

    void Refresh()
    {
        var theme = ThemeService.Current;
        spec = SurfaceChrome.For(theme.Roles, Kind);
        var fillColor = spec.Fill;
        var opacity = spec.FillOpacity;
        if (Hovered)
        {
            if (opacity < 0.05) { fillColor = theme.Roles.Surface; opacity = 0.9; }
            else fillColor = fillColor.Mixed(theme.Roles.Ink, 0.06);
        }
        fill.Background = theme.HighContrast ? theme.Brush("surface") : ThemeService.NewBrush(fillColor, opacity);
        fill.CornerRadius = new CornerRadius(Math.Min(spec.Radius, 999));
        // One ring per edge: uniform edges are one Border; a bevel is two (top-left, bottom-right).
        foreach (var ring in rings) Children.Remove(ring);
        rings.Clear();
        foreach (var edge in spec.Edges)
        {
            var tl = Decor();
            tl.BorderBrush = theme.HighContrast ? theme.Brush("line") : ThemeService.NewBrush(edge.TopLeft, edge.Opacity);
            rings.Add(tl);
            if (!edge.Uniform)
            {
                var br = Decor();
                br.BorderBrush = theme.HighContrast ? theme.Brush("line") : ThemeService.NewBrush(edge.BottomRight, edge.Opacity);
                rings.Add(br);
            }
        }
        var index = 1;
        foreach (var ring in rings) Children.Insert(index++, ring);
        InvalidateMeasure();
        InvalidateArrange();
    }

    protected override Size MeasureOverride(Size available)
    {
        var inset = Inset;
        var pad = Padding;
        double horizontal = inset * 2 + pad.Left + pad.Right, vertical = inset * 2 + pad.Top + pad.Bottom;
        foreach (var ring in rings) ring.Measure(available);
        fill.Measure(available);
        if (child is null) return new Size(horizontal, vertical);
        child.Measure(new Size(Math.Max(0, available.Width - horizontal), Math.Max(0, available.Height - vertical)));
        return new Size(child.DesiredSize.Width + horizontal, child.DesiredSize.Height + vertical);
    }

    protected override Size ArrangeOverride(Size final)
    {
        fill.Arrange(new Rect(0, 0, final.Width, final.Height));
        double offset = 0;
        var ringIndex = 0;
        var expected = spec.Edges.Sum(e => e.Uniform ? 1 : 2);
        foreach (var edge in rings.Count == expected ? spec.Edges : [])
        {
            var rect = new Rect(offset, offset, Math.Max(0, final.Width - offset * 2), Math.Max(0, final.Height - offset * 2));
            var w = edge.Width;
            var radius = new CornerRadius(Math.Max(0, Math.Min(spec.Radius, 999) - offset));
            var tl = rings[ringIndex++];
            if (edge.Uniform)
            {
                tl.BorderThickness = new Thickness(w);
                tl.CornerRadius = radius;
                tl.Arrange(rect);
            }
            else
            {
                var br = rings[ringIndex++];
                tl.BorderThickness = new Thickness(w, w, 0, 0);
                br.BorderThickness = new Thickness(0, 0, w, w);
                tl.Arrange(rect);
                br.Arrange(rect);
            }
            offset += w;
        }
        if (child is not null)
        {
            var pad = Padding;
            child.Arrange(new Rect(offset + pad.Left, offset + pad.Top,
                Math.Max(0, final.Width - offset * 2 - pad.Left - pad.Right),
                Math.Max(0, final.Height - offset * 2 - pad.Top - pad.Bottom)));
        }
        return final;
    }
}
