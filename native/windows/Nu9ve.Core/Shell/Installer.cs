namespace Nu9ve.Shell;

/// <summary>
/// What a suite Setup was asked to do (windows-app-v1 §9 «Instalador con identidad»). Pure so the rules can be
/// tested: no package means a labelled demonstration, never a silent no-op; `--quiet` needs a package.
/// </summary>
public sealed record InstallerArguments(string? Package, bool Quiet, string? Skin, bool Demo)
{
    public static InstallerArguments Parse(IReadOnlyList<string> args)
    {
        string? Value(string name)
        {
            for (var i = 0; i + 1 < args.Count; i++) if (args[i] == name) return args[i + 1];
            return null;
        }
        var package = Value("--package");
        var quiet = args.Contains("--quiet");
        return new(package, quiet, Value("--skin"), Demo: package is null);
    }

    /// <summary>Why these arguments cannot run, in words; null when they can.</summary>
    public string? Problem(Func<string, bool> exists)
    {
        if (Quiet && Package is null) return "--quiet necesita --package <ruta al .msix>";
        if (Package is not null && !Package.EndsWith(".msix", StringComparison.OrdinalIgnoreCase) && !Package.EndsWith(".msixbundle", StringComparison.OrdinalIgnoreCase))
            return $"{Package} no es un paquete .msix";
        if (Package is not null && !exists(Package)) return $"No existe {Package}";
        return null;
    }
}

/// <summary>Exit codes of a quiet install, stable for scripts.</summary>
public static class InstallerExit
{
    public const int Installed = 0, Failed = 1, InvalidArguments = 2, Cancelled = 3;
}

/// <summary>
/// Window outlines for suite Setups (a shaped, layered-free window via SetWindowRgn). Each app picks its own
/// shape from its identity; nüün's is the SD card of its icon: a rectangle with the top-left corner cut.
/// Points are in device pixels, clockwise from the top-left, for CreatePolygonRgn.
/// </summary>
public static class WindowShapes
{
    public static (int X, int Y)[] SdCard(int width, int height, int notch, int corner)
    {
        notch = Math.Clamp(notch, 0, Math.Min(width, height) / 2);
        corner = Math.Clamp(corner, 0, Math.Min(width, height) / 4);
        var points = new List<(int, int)> { (0, notch), (notch, 0), (width, 0) };
        // Small chamfers stand in for rounded corners: a polygon region has no antialiasing to soften arcs.
        if (corner > 0)
        {
            points.Clear();
            points.AddRange([(0, notch), (notch, 0), (width - corner, 0), (width, corner), (width, height - corner), (width - corner, height), (corner, height), (0, height - corner)]);
        }
        else points.AddRange([(width, height), (0, height)]);
        return [.. points];
    }
}
