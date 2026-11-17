using Nu9ve.Theme;
using Nuun.Core.Theme;
using Xunit;

namespace Nuun.Core.Tests;

// nu9ve-theme-v1 §5 y §7: la identidad es de cada app. Par de ThemeIdentityTests.swift (Nu9veTheme).
public class ThemeIdentityTests
{
    [Fact]
    public void TheThreeCommonsAreTheSameForEveryApp()
    {
        Assert.Equal([ThemeSkin.ClassicMac, ThemeSkin.Win9x, ThemeSkin.Terminal], ThemeSkinInfo.Commons);
        Assert.Equal(ThemeSkinInfo.Commons, ThemeSkinInfo.All.Where(s => s.IsCommon()));
        Assert.False(ThemeSkin.Glass.IsCommon());
        Assert.False(ThemeSkin.Gothic.IsCommon());
    }

    [Fact]
    public void PresetsAndDefaultsBelongToTheApp()
    {
        var lime = new ThemePreset("lime", "#B6E35A");
        Assert.Equal("#B6E35A", lime.Color.ToHex());
        // Another app's defaults: unknown or missing fields fall back to them, not to anyone else's.
        var other = ThemePreferences.For("#1d7a85", ThemeSkin.Gothic);
        Assert.Equal("#1D7A85", other.Base);
        Assert.Equal(other, ThemePreferences.Parse("{\"skin\": \"dither\", \"base\": \"nope\"}", other));
        Assert.Equal(ThemeSkin.Win9x, ThemePreferences.Parse("{\"skin\": \"win9x\"}", other).Skin);
    }

    [Fact]
    public void NuunKeepsItsAmberCristalAndGothic()
    {
        Assert.Equal("#FFBB5C", NuunTheme.Defaults.Base);
        Assert.Equal(NuunTheme.DefaultRoles().Seed.ToHex(), NuunTheme.Defaults.Seed.ToHex());
        Assert.Equal(ThemeSkinPart.Original, NuunTheme.Part(ThemeSkin.Glass));
        Assert.Equal(ThemeSkinPart.Contrast, NuunTheme.Part(ThemeSkin.Gothic));
        Assert.All(ThemeSkinInfo.Commons, s => Assert.Equal(ThemeSkinPart.Common, NuunTheme.Part(s)));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void CatalogSurfacesGiveTheSameRoles(bool dark)
    {
        foreach (var skin in ThemeSkinInfo.All)
        {
            var catalog = ThemeRoles.From(skin, dark, ThemeColor.Hex("#7CC6E6"));
            var given = new ThemeRoles(skin, catalog.Dark, catalog.Seed, ThemeSurfaces.For(skin, catalog.Dark, catalog.Seed));
            Assert.Equal(catalog.ByName(), given.ByName());
        }
    }

    [Fact]
    public void AnAppsOwnSkinKeepsTheContrastRules()
    {
        // An original outside the catalog (a paper tone): the app gives its surfaces.
        var paper = new ThemeSurfaces(ThemeColor.Hex("#F4EFE4"), ThemeColor.Hex("#FBF8F1"), ThemeColor.White, ThemeColor.Hex("#CFC6B4"), ThemeColor.Hex("#1D1B18"));
        var r = new ThemeRoles(ThemeSkin.Glass, false, ThemeColor.Hex("#B6E35A"), paper);
        foreach (var background in new[] { r.Canvas, r.Surface, r.Raised })
        {
            Assert.True(r.Ink.Contrast(background) >= 7);
            Assert.True(r.Muted.Contrast(background) >= 4.5);
            Assert.True(r.AccentText.Contrast(background) >= 4.5);
            Assert.True(r.Danger.Contrast(background) >= 4.5);
        }
        Assert.True(r.Ink.Contrast(r.AccentSoft) >= 7);
        Assert.Equal(ThemeColor.Hex("#F4EFE4"), r.Canvas);
    }
}
