using Nu9ve.Shell;
using Xunit;

namespace Nuun.Core.Tests;

// windows-app-v1 §9 «Instalador con identidad»: reglas puras del Setup de la suite.
public class InstallerTests
{
    [Fact]
    public void NoPackageIsALabelledDemonstration()
    {
        var args = InstallerArguments.Parse([]);
        Assert.True(args.Demo);
        Assert.Null(args.Problem(_ => false));
    }

    [Fact]
    public void QuietNeedsAnExistingMsix()
    {
        Assert.NotNull(InstallerArguments.Parse(["--quiet"]).Problem(_ => true));
        Assert.Contains("no es un paquete", InstallerArguments.Parse(["--package", "setup.exe"]).Problem(_ => true));
        Assert.Contains("No existe", InstallerArguments.Parse(["--package", "nuun.msix"]).Problem(_ => false));
        var ok = InstallerArguments.Parse(["--quiet", "--package", "nuun.msix", "--skin", "win9x"]);
        Assert.Null(ok.Problem(_ => true));
        Assert.False(ok.Demo);
        Assert.Equal("win9x", ok.Skin);
    }

    [Fact]
    public void ExitCodesAreStableForScripts()
    {
        Assert.Equal(0, InstallerExit.Installed);
        Assert.Equal(1, InstallerExit.Failed);
        Assert.Equal(2, InstallerExit.InvalidArguments);
        Assert.Equal(3, InstallerExit.Cancelled);
    }

    [Fact]
    public void SdCardOutlineCutsTheTopLeftCorner()
    {
        var points = WindowShapes.SdCard(600, 440, 64, 8);
        Assert.Equal((0, 64), points[0]);
        Assert.Equal((64, 0), points[1]);
        Assert.All(points, p => Assert.True(p.X is >= 0 and <= 600 && p.Y is >= 0 and <= 440));
        // Clockwise: the area by the shoelace formula is negative in screen coordinates (y down) → check sign.
        double area = 0;
        for (var i = 0; i < points.Length; i++)
        {
            var (x1, y1) = points[i];
            var (x2, y2) = points[(i + 1) % points.Length];
            area += x1 * (double)y2 - x2 * (double)y1;
        }
        Assert.True(area > 0, "clockwise on screen");
        // The cut removes a triangle of notch²/2 and three chamfers of corner²/2 (the fourth corner is the cut).
        Assert.Equal(600 * 440 - 64 * 64 / 2.0 - 3 * 8 * 8 / 2.0, area / 2, 0);
        // A notch larger than the window is clamped.
        Assert.Equal((0, 50), WindowShapes.SdCard(100, 100, 500, 0)[0]);
    }
}
