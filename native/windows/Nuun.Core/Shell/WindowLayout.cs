using Nu9ve.Shell;

namespace Nuun.Core.Shell;

/// <summary>One window, three presentations of the same state (docs/features/nuun-window.md).</summary>
public enum WindowMode { Compact, Selection, Full }

public static class WindowLayout
{
    // Mismos puntos que en Mac (440×700, 780×820, 1180×900). En Windows son DIPs; AppWindow los
    // multiplica por la escala del monitor (windows-app-v1 §2: mínimos reales por DPI).
    public static (double Width, double Height) Size(WindowMode mode) => mode switch
    {
        WindowMode.Compact => (440, 700),
        WindowMode.Selection => (780, 820),
        _ => (1180, 900),
    };

    /// <summary>Compact floats above other windows, like the Mac app; the rest stay normal.</summary>
    public static bool AlwaysOnTop(WindowMode mode) => mode == WindowMode.Compact;

    public static WindowMode Toggle(WindowMode mode) => mode == WindowMode.Compact ? WindowMode.Full : WindowMode.Compact;

    public static (int X, int Y, int Width, int Height) Fit((int X, int Y, int Width, int Height) frame, (int X, int Y, int Width, int Height) workArea) => Layout.Fit(frame, workArea);

    public static double Gutter(double width) => Layout.Gutter(width);
}

/// <summary>What the title bar keeps as it narrows. A pure rule so it can be tested (windows-app-v1 §1).</summary>
public sealed record TitleBarFit(bool NavigationLabels, bool Disks, bool Subtitle, bool Wordmark)
{
    // Ceden en el mismo orden que en Mac: primero los discos, luego el subtítulo, luego las etiquetas
    // de la navegación (quedan iconos con su nombre accesible); la marca es lo último.
    // Los umbrales están medidos para el español, que es más largo que el inglés (tema §9).
    public static TitleBarFit For(double width) => new(
        NavigationLabels: width >= 620,
        Disks: width >= 1260,
        Subtitle: width >= 760,
        Wordmark: width >= 360);
}
