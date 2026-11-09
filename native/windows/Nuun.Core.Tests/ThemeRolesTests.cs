using Nu9ve.Theme;
using Xunit;

namespace Nuun.Core.Tests;

// Port de native/MediaMenu/Tests/MediaMenuCoreTests/ThemeRolesTests.swift (nu9ve-theme-v1 §3, §11).
public class ThemeRolesTests
{
    public static IEnumerable<(double, double, double)> Seeds()
    {
        for (var hue = 0.0; hue < 1.0; hue += 1.0 / 12)
            foreach (var saturation in new[] { 0, 0.3, 0.64, 1.0 })
                foreach (var brightness in new[] { 0.15, 0.5, 0.8, 1.0 })
                    yield return (hue, saturation, brightness);
        foreach (var preset in ThemePreset.All) yield return preset.Color.Hsv;
    }

    public static TheoryData<ThemeSkin, bool> SkinsAndModes()
    {
        var data = new TheoryData<ThemeSkin, bool>();
        foreach (var skin in ThemeSkinInfo.All) { data.Add(skin, false); data.Add(skin, true); }
        return data;
    }

    [Theory, MemberData(nameof(SkinsAndModes))]
    public void ContrastMatrix(ThemeSkin skin, bool dark)
    {
        foreach (var (hue, saturation, brightness) in Seeds())
        {
            var r = new ThemeRoles(skin, dark, hue, saturation, brightness);
            var label = $"skin={skin} dark={dark} seed={r.Seed}";
            foreach (var background in new[] { r.Canvas, r.Surface, r.Raised })
            {
                Assert.True(r.Ink.Contrast(background) >= 7, $"{label} ink");
                Assert.True(r.Muted.Contrast(background) >= 4.5, $"{label} muted");
                Assert.True(r.AccentText.Contrast(background) >= 4.5, $"{label} accentText");
                foreach (var state in new[] { r.Danger, r.Warning, r.Success, r.Info })
                    Assert.True(state.Contrast(background) >= 4.5, $"{label} state {state}");
            }
            Assert.True(r.Ink.Contrast(r.WarningSoft) >= 7, $"{label} warningSoft ink");
            Assert.True(r.Warning.Contrast(r.WarningSoft) >= 4.5, $"{label} warningSoft warning");
            Assert.True(r.OnAccent.Contrast(r.Accent) >= 4.5, $"{label} onAccent");
            Assert.True(r.Ink.Contrast(r.AccentSoft) >= 7, $"{label} accentSoft");
        }
    }

    [Fact]
    public void DefaultSeedKeepsItsLook()
    {
        var light = ThemeRoles.Default(dark: false);
        Assert.Equal("#FFBB5C", light.Seed.ToHex());
        Assert.Equal(light.Seed.Mixed(ThemeColor.White, 0.80), light.Canvas);
        Assert.Equal(light.Seed.Mixed(ThemeColor.Black, 0.86), light.Ink);
        Assert.Equal(light.Seed.Mixed(ThemeColor.Black, 0.48), light.AccentText);
        Assert.Equal(ThemeColor.Black, light.OnAccent);
        Assert.Equal(ThemeRoles.Default(dark: true).Seed, ThemeRoles.Default(dark: true).AccentText);
    }

    [Fact]
    public void DarkSeedTextIsLiftedInDarkMode()
    {
        var r = new ThemeRoles(ThemeSkin.Glass, true, 240.0 / 360, 1, 0.15);
        Assert.True(r.Accent.Contrast(r.Canvas) < 2);
        Assert.True(r.AccentText.Contrast(r.Canvas) >= 4.5);
    }

    [Fact]
    public void PresetsRoundTripThroughHsv()
    {
        foreach (var preset in ThemePreset.All)
        {
            var (h, s, v) = preset.Color.Hsv;
            Assert.Equal(preset.Hex, ThemeColor.FromHsv(h, s, v).ToHex());
        }
        Assert.Equal(ThemePreset.All.Count, ThemePreset.All.Select(p => p.Id).Distinct().Count());
    }

    [Fact]
    public void PeriodSkinsKeepTheirEraAndGlassFollowsTheUser()
    {
        ThemeRoles R(ThemeSkin skin, bool dark) => new(skin, dark, ThemeRoles.DefaultHue, ThemeRoles.DefaultSaturation, 1);
        Assert.False(R(ThemeSkin.ClassicMac, true).Dark);
        Assert.False(R(ThemeSkin.Win9x, true).Dark);
        Assert.True(R(ThemeSkin.Gothic, false).Dark);
        Assert.True(R(ThemeSkin.Terminal, false).Dark);
        Assert.False(R(ThemeSkin.Glass, false).Dark);
        Assert.True(R(ThemeSkin.Glass, true).Dark);
        // windows-app-v1 §3: only Cristal keeps the rounded system window and Mica.
        Assert.Equal([ThemeSkin.Glass], ThemeSkinInfo.All.Where(s => s.RoundedWindow()));
        Assert.Equal([ThemeSkin.Glass], ThemeSkinInfo.All.Where(s => s.Backdrop() == WindowBackdrop.Mica));
        Assert.Null(ThemeSkinInfo.FromId("dither"));
    }

    [Fact]
    public void TerminalSignalKeepsTheSeedHue()
    {
        foreach (var hex in new[] { "#2E9E57", "#D52B1E", "#7CC6E6" })
        {
            var seed = ThemeColor.Hex(hex);
            var r = ThemeRoles.From(ThemeSkin.Terminal, false, seed);
            Assert.True(Math.Abs(r.Ink.Hsv.Hue - seed.Hsv.Hue) < 0.03, hex);
            Assert.True(r.Ink.Contrast(r.Canvas) >= 7);
        }
    }

    [Fact]
    public void StatesKeepTheirHueWhateverTheSeed()
    {
        var r = new ThemeRoles(ThemeSkin.Glass, false, 0, 1, 0.85);
        var hues = new[] { r.Danger, r.Warning, r.Success, r.Info }.Select(c => c.Hsv.Hue * 360).ToArray();
        Assert.True(hues[0] < 15 || hues[0] > 345);
        Assert.InRange(hues[1], 20, 45);
        Assert.InRange(hues[2], 120, 160);
        Assert.InRange(hues[3], 200, 230);
    }

    [Fact]
    public void ReadableKeepsColorsThatAlreadyRead()
    {
        var white = ThemeColor.Hex("#FFFFFF");
        var dark = ThemeColor.Hex("#222222");
        Assert.Equal(dark, dark.Readable([white]));
        Assert.True(ThemeColor.Hex("#FFEEAA").Readable([white]).Contrast(white) >= 4.5);
    }

    [Fact]
    public void Win9xCaptionKeepsWhiteTitleReadable()
    {
        foreach (var (h, s, v) in Seeds())
        {
            var (start, end) = new ThemeRoles(ThemeSkin.Win9x, false, h, s, v).Win9xCaption();
            Assert.True(ThemeColor.White.Contrast(start) >= 4.5);
            Assert.True(ThemeColor.White.Contrast(end) >= 4.5);
        }
    }
}

public class SurfaceChromeTests
{
    /// <summary>Every skinned control keeps its text readable (interaction §9: keycaps and buttons 4.5:1).</summary>
    [Theory, MemberData(nameof(ThemeRolesTests.SkinsAndModes), MemberType = typeof(ThemeRolesTests))]
    public void ForegroundReadsOnEveryKind(ThemeSkin skin, bool dark)
    {
        foreach (var (h, s, v) in ThemeRolesTests.Seeds())
        {
            var r = new ThemeRoles(skin, dark, h, s, v);
            foreach (var kind in Enum.GetValues<SurfaceKind>())
            {
                var spec = SurfaceChrome.For(r, kind);
                // A translucent fill sits on the canvas.
                var fill = r.Canvas.Mixed(spec.Fill, spec.FillOpacity);
                Assert.True(spec.Foreground.Contrast(fill) >= 4.5, $"{skin} dark={dark} seed={r.Seed} {kind}: {spec.Foreground} on {fill}");
            }
        }
    }

    [Fact]
    public void Win9xIsABevelAndCristalIsRound()
    {
        var win = SurfaceChrome.For(ThemeRoles.From(ThemeSkin.Win9x, false, ThemeColor.Hex("#FFBB5C")), SurfaceKind.Control);
        Assert.All(win.Edges, e => Assert.False(e.Uniform));
        Assert.Equal(0, win.Radius);
        var pressed = SurfaceChrome.For(ThemeRoles.From(ThemeSkin.Win9x, false, ThemeColor.Hex("#FFBB5C")), SurfaceKind.ControlPressed);
        Assert.Equal(1, pressed.PressOffset);
        var glass = SurfaceChrome.For(ThemeRoles.Default(), SurfaceKind.PillSelected);
        Assert.Equal(999, glass.Radius);
        Assert.Equal(ThemeRoles.Default().AccentSoft, glass.Fill);
    }
}
