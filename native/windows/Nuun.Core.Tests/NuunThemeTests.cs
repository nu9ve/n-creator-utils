using Nu9ve.Theme;
using Nuun.Core.Theme;
using Xunit;

namespace Nuun.Core.Tests;

// nu9ve-theme-v1 §7: la identidad de nüün sobre Nu9ve.Core. La matriz de contraste para cualquier semilla,
// los vectores compartidos, las superficies y las preferencias se prueban en nu9ve/core (windows/Nu9ve.Core.Tests);
// aquí, que nüün use la base con su ámbar, sus presets y su juego de skins.
public class NuunThemeTests
{
    public static TheoryData<ThemeSkin, bool> SkinsAndModes()
    {
        var data = new TheoryData<ThemeSkin, bool>();
        foreach (var skin in ThemeSkinInfo.All) { data.Add(skin, false); data.Add(skin, true); }
        return data;
    }

    [Fact]
    public void DefaultSeedKeepsItsLook()
    {
        var light = NuunTheme.DefaultRoles(dark: false);
        Assert.Equal("#FFBB5C", light.Seed.ToHex());
        Assert.Equal(light.Seed.Mixed(ThemeColor.White, 0.80), light.Canvas);
        Assert.Equal(light.Seed.Mixed(ThemeColor.Black, 0.86), light.Ink);
        Assert.Equal(light.Seed.Mixed(ThemeColor.Black, 0.48), light.AccentText);
        Assert.Equal(ThemeColor.Black, light.OnAccent);
        Assert.Equal(NuunTheme.DefaultRoles(dark: true).Seed, NuunTheme.DefaultRoles(dark: true).AccentText);
    }

    [Fact]
    public void PresetsRoundTripThroughHsv()
    {
        foreach (var preset in NuunTheme.Presets)
        {
            var (h, s, v) = preset.Color.Hsv;
            Assert.Equal(preset.Hex, ThemeColor.FromHsv(h, s, v).ToHex());
        }
        Assert.Equal(NuunTheme.Presets.Count, NuunTheme.Presets.Select(p => p.Id).Distinct().Count());
    }

    [Theory, MemberData(nameof(SkinsAndModes))]
    public void PresetsReadOnEverySkinAndSurface(ThemeSkin skin, bool dark)
    {
        foreach (var preset in NuunTheme.Presets)
        {
            var r = ThemeRoles.From(skin, dark, preset.Color);
            var label = $"{preset.Id} skin={skin} dark={dark}";
            foreach (var background in new[] { r.Canvas, r.Surface, r.Raised })
            {
                Assert.True(r.Ink.Contrast(background) >= 7, $"{label} ink");
                Assert.True(r.Muted.Contrast(background) >= 4.5, $"{label} muted");
                Assert.True(r.AccentText.Contrast(background) >= 4.5, $"{label} accentText");
            }
            Assert.True(r.OnAccent.Contrast(r.Accent) >= 4.5, $"{label} onAccent");
            foreach (var kind in Enum.GetValues<SurfaceKind>())
            {
                var spec = SurfaceChrome.For(r, kind);
                var fill = r.Canvas.Mixed(spec.Fill, spec.FillOpacity);
                Assert.True(spec.Foreground.Contrast(fill) >= 4.5, $"{label} {kind}");
            }
        }
    }

    [Fact]
    public void NuunKeepsItsAmberCristalAndGothic()
    {
        Assert.Equal("#FFBB5C", NuunTheme.Defaults.Base);
        Assert.Equal(NuunTheme.DefaultRoles().Seed.ToHex(), NuunTheme.Defaults.Seed.ToHex());
        Assert.Equal(ThemeSkinPart.Original, NuunTheme.Part(ThemeSkin.Glass));
        Assert.Equal(ThemeSkinPart.Contrast, NuunTheme.Part(ThemeSkin.Gothic));
        Assert.All(ThemeSkinInfo.Commons, s => Assert.Equal(ThemeSkinPart.Common, NuunTheme.Part(s)));
        // A saved choice from before keeps working with nüün's defaults.
        var saved = ThemePreferences.Parse("""{"version":1,"skin":"win9x","base":"#7CC6E6","mode":"dark"}""", NuunTheme.Defaults);
        Assert.Equal(ThemeSkin.Win9x, saved.Skin);
        Assert.Equal(NuunTheme.Defaults with { Skin = ThemeSkin.Win9x, Base = "#7CC6E6", Mode = ThemeMode.Dark }, saved);
    }
}
