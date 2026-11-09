using System.Text.Json;
using System.Text.Json.Nodes;
using Nu9ve.Theme;
using Xunit;

namespace Nuun.Core.Tests;

/// <summary>
/// nu9ve-theme-v1 §3 «Conviene: compartir vectores de prueba». The file holds, per skin × mode × base,
/// the hex of every common role. C# checks it here; Swift (MediaMenuCoreTests) and TypeScript should read
/// the same file. Regenerate only on purpose: `$env:NU9VE_WRITE_THEME_VECTORS = "1"; dotnet test`.
/// </summary>
public class ThemeVectorsTests
{
    static readonly string[] Bases = ["#FFBB5C", "#D52B1E", "#8A99A8", "#7CC6E6", "#808080", "#FFFF00", "#0A0A0A", "#F5F5F5", "#2F78D6", "#D66154", "#1D7A85"];

    static string VectorsPath()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir is not null && !Directory.Exists(Path.Combine(dir.FullName, "docs", "contracts"))) dir = dir.Parent;
        Assert.NotNull(dir);
        return Path.Combine(dir!.FullName, "docs", "contracts", "examples", "theme-vectors-v1.json");
    }

    static JsonObject Generate()
    {
        var cases = new JsonArray();
        foreach (var skin in ThemeSkinInfo.All)
            foreach (var dark in new[] { false, true })
                foreach (var hex in Bases)
                {
                    var roles = ThemeRoles.From(skin, dark, ThemeColor.Hex(hex));
                    var expected = new JsonObject();
                    foreach (var (name, color) in roles.ByName()) expected[name] = color.ToHex();
                    cases.Add(new JsonObject { ["skin"] = skin.Id(), ["dark"] = dark, ["base"] = hex, ["resolvedDark"] = roles.Dark, ["roles"] = expected });
                }
        return new JsonObject
        {
            ["contract"] = "nu9ve-theme-v1 §3",
            ["input"] = "base hex → HSV (ThemeColor.hsv) → ThemeRoles(skin, dark, hue, saturation, value)",
            ["source"] = "n-creator-utils native/windows Nuun.Core (port of MediaMenuCore ThemeColor.swift)",
            ["verified"] = new JsonArray("csharp"),
            ["cases"] = cases,
        };
    }

    [Fact]
    public void SharedVectorsMatchThisImplementation()
    {
        var path = VectorsPath();
        var generated = Generate();
        if (Environment.GetEnvironmentVariable("NU9VE_WRITE_THEME_VECTORS") == "1")
        {
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            File.WriteAllText(path, generated.ToJsonString(new JsonSerializerOptions { WriteIndented = true, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping }) + "\n");
        }
        Assert.True(File.Exists(path), $"Missing {path}; run once with NU9VE_WRITE_THEME_VECTORS=1");
        var stored = JsonNode.Parse(File.ReadAllText(path))!["cases"]!.AsArray();
        var expected = generated["cases"]!.AsArray();
        Assert.Equal(expected.Count, stored.Count);
        for (var i = 0; i < expected.Count; i++)
            Assert.True(JsonNode.DeepEquals(expected[i], stored[i]), $"case {i}: {stored[i]?.ToJsonString()} != {expected[i]?.ToJsonString()}");
    }
}
