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

    /// <summary>
    /// The id of a skin of the app's own (an original or contrast outside the catalog, nu9ve-theme-v1 §5), or null for a
    /// catalog skin. It is saved as `skin`; <see cref="Skin"/> then keeps the catalog skin that lends its metrics.
    /// </summary>
    public string? AppSkin { get; init; }

    /// <summary>The saved `skin` id: the app's own, or the catalog skin's.</summary>
    public string SkinId => AppSkin ?? Skin.Id();

    /// <summary>An app's defaults: its own base color and original skin, everything else automatic.</summary>
    public static ThemePreferences For(string baseHex, ThemeSkin skin = ThemeSkin.Glass, string? appSkin = null) =>
        new(skin, ThemeColor.Hex(baseHex).ToHex(), ThemeMode.System, ThemeContrast.Neutral, "auto", "auto", false) { AppSkin = appSkin };

    /// <summary>The choice with a catalog skin.</summary>
    public ThemePreferences WithSkin(ThemeSkin skin) => this with { Skin = skin, AppSkin = null };

    /// <summary>The choice with one of the app's own skins; <paramref name="metrics"/> is the catalog skin it borrows from.</summary>
    public ThemePreferences WithAppSkin(string id, ThemeSkin metrics) => this with { Skin = metrics, AppSkin = id };

    public ThemeColor Seed => ThemeColor.Hex(Base);

    /// <summary>Roles for this choice. <paramref name="systemDark"/> answers «Sistema».</summary>
    public ThemeRoles Roles(bool systemDark) =>
        ThemeRoles.From(Skin, Mode switch { ThemeMode.Dark => true, ThemeMode.Light => false, _ => systemDark }, Seed);

    static readonly string[] Textures = ["auto", "dither", "smooth", "flat"];
    static readonly string[] Typographies = ["auto", "display", "pixel", "mono", "system"];

    /// <summary>
    /// <paramref name="defaults"/> are the app's (<see cref="For"/>): what a missing or unknown field becomes.
    /// <paramref name="appSkins"/> are the ids of the app's own skins; any other id must be a catalog skin.
    /// </summary>
    public static ThemePreferences Parse(string? json, ThemePreferences defaults, IReadOnlyCollection<string>? appSkins = null)
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
        var skinId = Text("skin");
        var parsed = new ThemePreferences(defaults.Skin, baseHex, mode, contrast, texture, typography, sounds) { AppSkin = defaults.AppSkin };
        if (skinId is not null && appSkins is not null && appSkins.Contains(skinId))
            return parsed with { AppSkin = skinId };
        return ThemeSkinInfo.FromId(skinId) is { } catalog ? parsed.WithSkin(catalog) : parsed;
    }

    public string ToJson()
    {
        var obj = new JsonObject
        {
            ["version"] = Version,
            ["skin"] = SkinId,
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
