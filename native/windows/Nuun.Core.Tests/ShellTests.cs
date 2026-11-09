using Nu9ve.Shell;
using Nuun.Core.Shell;
using Nu9ve.Theme;
using Xunit;

namespace Nuun.Core.Tests;

public class ThemePreferencesTests
{
    [Fact]
    public void UnknownFieldsFallBackOneByOne()
    {
        var parsed = ThemePreferences.Parse("""{"version":1,"skin":"dither","base":"#D52B1E","mode":"sepia","contrast":"high","texture":"plasma","typography":"mono"}""");
        Assert.Equal(ThemeSkin.Glass, parsed.Skin);            // retired skin → default
        Assert.Equal("#D52B1E", parsed.Base);                  // kept
        Assert.Equal(ThemeMode.System, parsed.Mode);           // unknown → default
        Assert.Equal(ThemeContrast.High, parsed.Contrast);     // kept
        Assert.Equal("auto", parsed.Texture);
        Assert.Equal("mono", parsed.Typography);
    }

    [Fact]
    public void BrokenJsonIsTheDefaultAndRoundTripIsExact()
    {
        Assert.Equal(ThemePreferences.Default, ThemePreferences.Parse("{not json"));
        Assert.Equal(ThemePreferences.Default, ThemePreferences.Parse(null));
        var choice = ThemePreferences.Default with { Skin = ThemeSkin.Win9x, Base = "#7CC6E6", Mode = ThemeMode.Dark, InterfaceSounds = true };
        Assert.Equal(choice, ThemePreferences.Parse(choice.ToJson()));
        Assert.Contains("\"skin\": \"win9x\"", choice.ToJson());
    }

    [Fact]
    public void SystemModeFollowsTheSystemUnlessTheSkinFixesIt()
    {
        var system = ThemePreferences.Default;
        Assert.True(system.Roles(systemDark: true).Dark);
        Assert.False(system.Roles(systemDark: false).Dark);
        Assert.False((system with { Skin = ThemeSkin.Win9x }).Roles(systemDark: true).Dark);
    }
}

public class ShortcutCatalogTests
{
    [Fact]
    public void IdsAreUniqueAndNoActionsShareAChord()
    {
        Assert.Equal(ShortcutCatalog.All.Count, ShortcutCatalog.All.Select(s => s.Id).Distinct().Count());
        Assert.Empty(ShortcutCatalog.Conflicts());
    }

    [Fact]
    public void NoShortcutTakesAKeyOfTheSystem()
    {
        foreach (var shortcut in ShortcutCatalog.All.Where(s => s.Keys is not null))
            Assert.DoesNotContain(shortcut.Keys!.Value, ShortcutCatalog.Reserved);
    }

    [Fact]
    public void SuiteShortcutsUseTheirWindowsKeys()
    {
        // nu9ve-interaction-v1 §8 translated by windows-app-v1 §4.
        Assert.Equal("Ctrl+,", ShortcutCatalog.Get("app.settings").Keys!.Value.Label);
        Assert.Equal("Ctrl+/", ShortcutCatalog.Get("app.shortcuts").Keys!.Value.Label);
        Assert.Equal("F1", ShortcutCatalog.Get("app.help").Keys!.Value.Label);
        Assert.Equal("F11", ShortcutCatalog.Get("app.fullscreen").Keys!.Value.Label);
        Assert.Equal("Ctrl+\\", ShortcutCatalog.Get("app.mode").Keys!.Value.Label);
        var navigation = ShortcutCatalog.All.Where(s => s.Id.StartsWith("nav.", StringComparison.Ordinal)).Select(s => s.Keys!.Value.Label);
        Assert.Equal(["Ctrl+1", "Ctrl+2", "Ctrl+3", "Ctrl+4"], navigation);
    }

    [Fact]
    public void EveryActionExplainsItself()
    {
        Assert.All(ShortcutCatalog.All, s =>
        {
            Assert.False(string.IsNullOrWhiteSpace(s.Title));
            Assert.False(string.IsNullOrWhiteSpace(s.TitleEn));
            Assert.False(string.IsNullOrWhiteSpace(s.Help));
            Assert.InRange(s.Keycap.Length, 1, 3);
        });
    }
}

public class WindowLayoutTests
{
    [Fact]
    public void ModesMatchTheMacSizes()
    {
        Assert.Equal((440, 700), WindowLayout.Size(WindowMode.Compact));
        Assert.Equal((780, 820), WindowLayout.Size(WindowMode.Selection));
        Assert.Equal((1180, 900), WindowLayout.Size(WindowMode.Full));
        Assert.True(WindowLayout.AlwaysOnTop(WindowMode.Compact));
        Assert.False(WindowLayout.AlwaysOnTop(WindowMode.Full));
    }

    [Fact]
    public void RememberedFrameStaysOnAnAvailableMonitor()
    {
        var work = (0, 0, 1280, 720);
        Assert.Equal((100, 0, 1180, 720), WindowLayout.Fit((100, 20, 1180, 900), work));
        Assert.Equal((840, 0, 440, 700), WindowLayout.Fit((3000, -50, 440, 700), work));
    }

    [Fact]
    public void TitleBarGivesWayInOrder()
    {
        TitleBarFit? previous = null;
        foreach (var width in Enumerable.Range(30, 120).Select(i => i * 10.0))
        {
            var fit = TitleBarFit.For(width);
            // Disks go before the subtitle, the subtitle before labels, labels before the wordmark.
            if (fit.Disks) Assert.True(fit.Subtitle);
            if (fit.Subtitle) Assert.True(fit.NavigationLabels);
            if (fit.NavigationLabels) Assert.True(fit.Wordmark);
            if (previous is not null)
            {
                // Widening never hides anything.
                Assert.True(!previous.Disks || fit.Disks);
                Assert.True(!previous.NavigationLabels || fit.NavigationLabels);
            }
            previous = fit;
        }
        Assert.Equal(16, WindowLayout.Gutter(440));
        Assert.Equal(32, WindowLayout.Gutter(1180));
    }

    [Fact]
    public void TreemapIsProportionalAndInside()
    {
        var bounds = new Treemap.Rect(0, 0, 400, 200);
        double[] values = [50, 30, 15, 5, 0];
        var rects = Treemap.Layout(values, bounds);
        var area = bounds.Width * bounds.Height;
        for (var i = 0; i < 4; i++)
        {
            Assert.Equal(values[i] / 100 * area, rects[i].Width * rects[i].Height, 3);
            Assert.True(rects[i].X >= 0 && rects[i].Y >= 0 && rects[i].X + rects[i].Width <= 400.001 && rects[i].Y + rects[i].Height <= 200.001);
        }
        Assert.Equal(default, rects[4]);
    }

    [Fact]
    public void BytesUseDecimalUnits()
    {
        Assert.Equal("4.2 MB", Presentation.Bytes(4_200_000));
        Assert.Equal("2.1 TB", Presentation.Bytes(2_140_000_000_000));
        Assert.Equal("—", Presentation.Bytes(null));
    }
}
