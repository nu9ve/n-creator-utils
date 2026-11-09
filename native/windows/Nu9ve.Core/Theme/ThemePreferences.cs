using System.Text.Json;
using System.Text.Json.Nodes;

namespace Nu9ve.Theme;

public enum ThemeMode { System, Light, Dark }
public enum ThemeContrast { Neutral, Tonal, High }

// nu9ve-theme-v1 §4 · origen: curado src/lib/theme.ts validateTheme (validación campo por campo).
// Formato guardado común: {version, skin, base, mode, contrast, texture, typography, app}.
// Divergencia: nüün todavía no aplica Contraste ni Textura (ocultas en Apariencia, §4: «si una app no
// soporta una opción, la oculta»); se leen y se conservan para no perder la elección al migrar.
/// <summary>The user's appearance choice. Unknown values fall back per field, never the whole record.</summary>
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
    public static readonly ThemePreferences Default = new(ThemeSkin.Glass, "#FFBB5C", ThemeMode.System, ThemeContrast.Neutral, "auto", "auto", false);

    public ThemeColor Seed => ThemeColor.FromHex(Base) ?? ThemeColor.Hex(Default.Base);

    /// <summary>Roles for this choice. <paramref name="systemDark"/> answers «Sistema».</summary>
    public ThemeRoles Roles(bool systemDark) =>
        ThemeRoles.From(Skin, Mode switch { ThemeMode.Dark => true, ThemeMode.Light => false, _ => systemDark }, Seed);

    static readonly string[] Textures = ["auto", "dither", "smooth", "flat"];
    static readonly string[] Typographies = ["auto", "display", "pixel", "mono", "system"];

    public static ThemePreferences Parse(string? json)
    {
        if (string.IsNullOrWhiteSpace(json)) return Default;
        JsonNode? node;
        try { node = JsonNode.Parse(json); } catch (JsonException) { return Default; }
        if (node is not JsonObject obj) return Default;
        string? Text(string key) => obj[key] is JsonValue v && v.TryGetValue<string>(out var s) ? s : null;
        var baseHex = ThemeColor.FromHex(Text("base"))?.ToHex() ?? Default.Base;
        var mode = Text("mode") switch { "light" => ThemeMode.Light, "dark" => ThemeMode.Dark, "system" => ThemeMode.System, _ => Default.Mode };
        var contrast = Text("contrast") switch { "tonal" => ThemeContrast.Tonal, "high" => ThemeContrast.High, "neutral" => ThemeContrast.Neutral, _ => Default.Contrast };
        var texture = Text("texture") is { } t && Textures.Contains(t) ? t : Default.Texture;
        var typography = Text("typography") is { } y && Typographies.Contains(y) ? y : Default.Typography;
        var sounds = obj["app"]?["interfaceSounds"] is JsonValue sv && sv.TryGetValue<bool>(out var b) ? b : Default.InterfaceSounds;
        return new(ThemeSkinInfo.FromId(Text("skin")) ?? Default.Skin, baseHex, mode, contrast, texture, typography, sounds);
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
