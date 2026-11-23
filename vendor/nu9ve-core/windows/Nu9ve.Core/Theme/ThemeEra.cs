namespace Nu9ve.Theme;

// nu9ve-theme-v1 §4 «Época» · par de Nu9veTheme ThemeEra.swift (nu9ve/core).
/// <summary>
/// How a period skin (Macintosh, Windows 9x) takes the user's color. <see cref="Original"/> is the period's own palette
/// and the default; <see cref="Tinted"/> takes the user's color for paper, lines and ink, and the user's mode. The skin's
/// measures never change.
/// </summary>
public enum ThemeEra { Original, Tinted }

public static class ThemeEraInfo
{
    public static string Id(this ThemeEra era) => era == ThemeEra.Tinted ? "tinted" : "original";

    public static ThemeEra? FromId(string? id) => id switch
    {
        "original" => ThemeEra.Original,
        "tinted" => ThemeEra.Tinted,
        _ => null,
    };
}
