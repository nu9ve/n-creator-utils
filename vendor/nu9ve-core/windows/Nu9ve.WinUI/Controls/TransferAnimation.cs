using Rectangle = Microsoft.UI.Xaml.Shapes.Rectangle;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Nu9ve.Theme;
using Nu9ve.WinUI;

namespace Nu9ve.WinUI.Controls;

// nu9ve-interaction-v1 §7 «Acciones largas» · origen: MediaMenu ActionProgress.swift (documentos que viajan
// entre discos, a 12 cuadros por segundo, pausado con la app inactiva y quieto con Reducir movimiento).
// Divergencia: una sola figura para todos los skins (papeles entre dos discos) con las medidas y colores del skin;
// las variantes de época (hormigas de Macintosh, `[====>···]` de Terminal) quedan pendientes.
/// <summary>Decorative: sheets travel from the source disk to the destination. Colors are state colors (info = copy).</summary>
public sealed partial class TransferAnimation : Canvas
{
    readonly Microsoft.UI.Xaml.DispatcherTimer timer = new() { Interval = TimeSpan.FromMilliseconds(1000.0 / 12) };
    readonly List<NuSurface> sheets = [];
    double phase;

    public bool Running { get => timer.IsEnabled; set { if (value && ThemeService.Current.AnimationsEnabled) timer.Start(); else timer.Stop(); } }

    public TransferAnimation()
    {
        Height = 72;
        IsHitTestVisible = false;
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
        timer.Tick += (_, _) => { phase = (phase + 0.03) % 1; Place(); };
        Loaded += (_, _) => Build();
        Unloaded += (_, _) => timer.Stop();
        SizeChanged += (_, _) => Place();
    }

    void Build()
    {
        Children.Clear();
        sheets.Clear();
        var info = ThemeService.Current.Brush("info");
        foreach (var x in new[] { 0.0, 1.0 })
        {
            var disk = UI.Surface(SurfaceKind.Well, UI.Icon("", 22, x == 0 ? "muted" : "success"), 10);
            disk.Tag = x;
            Children.Add(disk);
        }
        for (var i = 0; i < 3; i++)
        {
            var sheet = new NuSurface { Kind = SurfaceKind.Control, Width = 18, Height = 22, Child = new Rectangle { Height = 2, Fill = info, Margin = new Thickness(3, 6, 3, 0), VerticalAlignment = VerticalAlignment.Top } };
            sheets.Add(sheet);
            Children.Add(sheet);
        }
        Place();
    }

    void Place()
    {
        var width = ActualWidth;
        if (width <= 0) return;
        foreach (var disk in Children.OfType<NuSurface>().Where(d => d.Tag is double))
        {
            SetLeft(disk, (double)disk.Tag! * (width - 52));
            SetTop(disk, 14);
        }
        for (var i = 0; i < sheets.Count; i++)
        {
            var t = (phase + i / 3.0) % 1;
            // A gentle arc between the two disks.
            SetLeft(sheets[i], 52 + t * (width - 122));
            SetTop(sheets[i], 24 - Math.Sin(t * Math.PI) * 18);
            sheets[i].Opacity = Math.Sin(t * Math.PI);
        }
    }
}
