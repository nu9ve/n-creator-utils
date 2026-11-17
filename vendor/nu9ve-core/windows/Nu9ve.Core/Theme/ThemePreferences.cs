using System.Text.Json;
using System.Text.Json.Nodes;

namespace Nu9ve.Theme;

public enum ThemeMode { System, Light, Dark }
public enum ThemeContrast { Neutral, Tonal, High }

// nu9ve-theme-v1 §4 · validación campo por campo, como la del tema de la web.
// Formato guardado común: {version, skin, base, mode, contrast, texture, typography, app}.
// Una app que todavía no aplica una opción la oculta en Apariencia (§4) y aun así la lee y la conserva,
// para no perder la elección al migrar. El color y el skin por omisión son de cada app (§7).
/// <summary>The user's appearance choice. Unknown values fall back per field to the app's defaults, never the whole record.</summary>
public sealed record ThemePreferences(
    ThemeSkin Skin,
    string Base,
    ThemeMode Mode,
    ThemeContrast Contrast,
    string Texture,
    string Typography,
    bool InterfaceSounds)
{
    public const int Version = 1;

    /// <summary>An app's defaults: its own base color and original skin, everything else automatic.</summary>
    public static ThemePreferences For(string baseHex, ThemeSkin skin = ThemeSkin.Glass) =>
        new(skin, ThemeColor.Hex(baseHex).ToHex(), ThemeMode.System, ThemeContrast.Neutral, "auto", "auto", false);

    public ThemeColor Seed => ThemeColor.Hex(Base);

    /// <summary>Roles for this choice. <paramref name="systemDark"/> answers «Sistema».</summary>
    public ThemeRoles Roles(bool systemDark) =>
        ThemeRoles.From(Skin, Mode switch { ThemeMode.Dark => true, ThemeMode.Light => false, _ => systemDark }, Seed);

    static readonly string[] Textures = ["auto", "dither", "smooth", "flat"];
    static readonly string[] Typographies = ["auto", "display", "pixel", "mono", "system"];

    /// <summary><paramref name="defaults"/> are the app's (<see cref="For"/>): what a missing or unknown field becomes.</summary>
    public static ThemePreferences Parse(string? json, ThemePreferences defaults)
    {
        if (string.IsNullOrWhiteSpace(json)) return defaults;
        JsonNode? node;
        try { node = JsonNode.Parse(json); } catch (JsonException) { return defaults; }
        if (node is not JsonObject obj) return defaults;
        string? Text(string key) => obj[key] is JsonValue v && v.TryGetValue<string>(out var s) ? s : null;
        var baseHex = ThemeColor.FromHex(Text("base"))?.ToHex() ?? defaults.Base;
        var mode = Text("mode") switch { "light" => ThemeMode.Light, "dark" => ThemeMode.Dark, "system" => ThemeMode.System, _ => defaults.Mode };
        var contrast = Text("contrast") switch { "tonal" => ThemeContrast.Tonal, "high" => ThemeContrast.High, "neutral" => ThemeContrast.Neutral, _ => defaults.Contrast };
        var texture = Text("texture") is { } t && Textures.Contains(t) ? t : defaults.Texture;
        var typography = Text("typography") is { } y && Typographies.Contains(y) ? y : defaults.Typography;
        var sounds = obj["app"]?["interfaceSounds"] is JsonValue sv && sv.TryGetValue<bool>(out var b) ? b : defaults.InterfaceSounds;
        return new(ThemeSkinInfo.FromId(Text("skin")) ?? defaults.Skin, baseHex, mode, contrast, texture, typography, sounds);
    }

    public string ToJson()
    {
        var obj = new JsonObject
        {
            ["version"] = Version,
            ["skin"] = Skin.Id(),
            ["base"] = Seed.ToHex(),
            ["mode"] = Mode.ToString().ToLowerInvariant(),
            ["contrast"] = Contrast.ToString().ToLowerInvariant(),
            ["texture"] = Texture,
            ["typography"] = Typography,
            ["app"] = new JsonObject { ["interfaceSounds"] = InterfaceSounds },
        };
        return obj.ToJsonString(new JsonSerializerOptions { WriteIndented = true });
    }
}
