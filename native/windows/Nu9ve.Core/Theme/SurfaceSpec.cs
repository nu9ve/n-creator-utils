namespace Nu9ve.Theme;

/// <summary>What a skinned surface is for. Views pick the kind; the skin decides how it looks (theme §1).</summary>
/// <remarks>`Group` holds a row of pills (navigation, steps): the Mac app's capsule on glass.</remarks>
public enum SurfaceKind { Panel, Well, Control, ControlPressed, Primary, PrimaryPressed, Pill, PillSelected, Danger, Group, Card, CardSelected }

/// <summary>One ring of a surface's edge: top-left and bottom-right colors (equal unless it is a bevel).</summary>
public readonly record struct Edge(ThemeColor TopLeft, ThemeColor BottomRight, double Width, double Opacity = 1)
{
    public bool Uniform => TopLeft == BottomRight;
    public static Edge Line(ThemeColor color, double width, double opacity = 1) => new(color, color, width, opacity);
}

/// <summary>
/// The drawing of a surface for the current roles: fill, radius, edges from outside in, text color and
/// how far the content moves when pressed. Pure: the WinUI layer only turns it into Borders.
/// </summary>
public sealed record SurfaceSpec(ThemeColor Fill, double FillOpacity, double Radius, IReadOnlyList<Edge> Edges, ThemeColor Foreground, bool Strong, double PressOffset);

// nu9ve-theme-v1 §5 y §8 · origen: native/MediaMenu Appearance.swift CreatorButtonStyle y SkinChrome.swift
// (Win9xBevel, borde de 1 bit, losa gótica, `[K]` terminal). Una sola decisión por skin, aquí, para que
// ninguna vista de Windows pregunte qué skin está activo (§1).
public static class SurfaceChrome
{
    // Windows 9x bevel colors (Fractos ThemeChrome `Win9x`): highlight, light shadow, dark shadow.
    static readonly ThemeColor Highlight = ThemeColor.White;
    static readonly ThemeColor LightShadow = new(128 / 255.0, 128 / 255.0, 128 / 255.0);
    static readonly ThemeColor DarkShadow = new(10 / 255.0, 10 / 255.0, 10 / 255.0);

    static Edge[] Bevel(bool raised) => raised
        ? [new(Highlight, DarkShadow, 1), new(ThemeColor.Hex("#DFDFDF"), LightShadow, 1)]
        : [new(LightShadow, Highlight, 1), new(DarkShadow, ThemeColor.Hex("#DFDFDF"), 1)];

    /// <summary>A pressed fill differs from the accent, so its text is chosen again on it (caught by SurfaceChromeTests).</summary>
    static SurfaceSpec Pressed(ThemeColor fill, double radius, Edge[] edges) => new(fill, 1, radius, edges, fill.OnColor, true, 0);

    public static SurfaceSpec For(ThemeRoles r, SurfaceKind kind)
    {
        // A selectable card (a source, a skin): the panel's shape; selected = accentSoft with an accent ring, the same in every skin.
        if (kind is SurfaceKind.Card or SurfaceKind.CardSelected)
        {
            var panel = For(r, SurfaceKind.Panel);
            return kind == SurfaceKind.Card ? panel
                : panel with { Fill = r.AccentSoft, FillOpacity = 1, Edges = [Edge.Line(r.AccentText, 2)], Foreground = r.Ink, Strong = false };
        }
        var skin = r.Skin;
        var lineOpacity = r.LineOpacity;
        var radius = kind switch
        {
            SurfaceKind.Panel or SurfaceKind.Well => skin.PanelRadius(),
            SurfaceKind.Pill or SurfaceKind.PillSelected or SurfaceKind.Group => skin.PillRadius(),
            _ => skin.ControlRadius(),
        };
        switch (skin)
        {
            case ThemeSkin.ClassicMac:
                return kind switch
                {
                    SurfaceKind.Panel => new(r.Surface, 1, radius, [Edge.Line(r.Ink, 1)], r.Ink, false, 0),
                    SurfaceKind.Well => new(r.Raised, 1, radius, [Edge.Line(r.Ink, 1)], r.Ink, false, 0),
                    // Pressed inverts; the default button carries the period's heavy outer ring.
                    SurfaceKind.ControlPressed or SurfaceKind.PrimaryPressed => new(r.Ink, 1, radius, [Edge.Line(r.Ink, 1)], ThemeColor.White, kind == SurfaceKind.PrimaryPressed, 0),
                    SurfaceKind.Primary => new(r.Surface, 1, radius, [Edge.Line(r.Ink, 2), Edge.Line(r.Surface, 2), Edge.Line(r.Ink, 1)], r.AccentText, true, 0),
                    SurfaceKind.PillSelected => new(r.Ink, 1, radius, [Edge.Line(r.Ink, 1)], ThemeColor.White, true, 0),
                    SurfaceKind.Pill => new(r.Surface, 1, radius, [Edge.Line(r.Ink, 1)], r.Ink, false, 0),
                    SurfaceKind.Danger => new(r.Surface, 1, radius, [Edge.Line(r.Ink, 1)], r.Danger, false, 0),
                    _ => new(r.Surface, 1, radius, [Edge.Line(r.Ink, 1)], r.Ink, false, 0),
                };
            case ThemeSkin.Win9x:
                return kind switch
                {
                    SurfaceKind.Panel => new(r.Canvas, 1, 0, Bevel(true), r.Ink, false, 0),
                    SurfaceKind.Well => new(ThemeColor.White, 1, 0, Bevel(false), r.Ink, false, 0),
                    SurfaceKind.ControlPressed or SurfaceKind.PrimaryPressed or SurfaceKind.PillSelected =>
                        new(kind == SurfaceKind.PillSelected ? r.Surface : r.Canvas, 1, 0,
                            kind == SurfaceKind.PrimaryPressed ? [Edge.Line(ThemeColor.Black, 1), .. Bevel(false)] : Bevel(false),
                            r.Ink, kind != SurfaceKind.ControlPressed, kind == SurfaceKind.PillSelected ? 0 : 1),
                    SurfaceKind.Primary => new(r.Canvas, 1, 0, [Edge.Line(ThemeColor.Black, 1), .. Bevel(true)], r.Ink, true, 0),
                    SurfaceKind.Danger => new(r.Canvas, 1, 0, Bevel(true), r.Danger, false, 0),
                    _ => new(r.Canvas, 1, 0, Bevel(true), r.Ink, false, 0),
                };
            case ThemeSkin.Terminal:
                return kind switch
                {
                    SurfaceKind.Panel => new(r.Surface, 1, 0, [Edge.Line(r.Line, 1)], r.Ink, false, 0),
                    SurfaceKind.Well => new(r.Canvas, 1, 0, [Edge.Line(r.Line, 1)], r.Ink, false, 0),
                    SurfaceKind.Primary or SurfaceKind.PillSelected => new(r.Accent, 1, 0, [Edge.Line(r.AccentText, 1)], r.OnAccent, true, 0),
                    SurfaceKind.PrimaryPressed or SurfaceKind.ControlPressed => new(r.AccentText, 1, 0, [Edge.Line(r.AccentText, 1)], r.Canvas, true, 0),
                    SurfaceKind.Danger => new(r.Surface, 1, 0, [Edge.Line(r.Danger, 1)], r.Danger, false, 0),
                    _ => new(r.Surface, 1, 0, [Edge.Line(r.Line, 1)], r.Ink, false, 0),
                };
            case ThemeSkin.Gothic:
                return kind switch
                {
                    // Double frame on brand panels: a line and the accent at half strength.
                    SurfaceKind.Panel => new(r.Surface, 1, radius, [Edge.Line(r.Line, 1, lineOpacity), Edge.Line(r.Surface, 2), Edge.Line(r.Accent, 1, 0.35)], r.Ink, false, 0),
                    SurfaceKind.Well => new(r.Canvas, 1, radius, [Edge.Line(r.Line, 1, lineOpacity)], r.Ink, false, 0),
                    SurfaceKind.Primary or SurfaceKind.PillSelected => new(r.Accent, 1, radius, [Edge.Line(r.Ink, 1, 0.35)], r.OnAccent, true, 0),
                    SurfaceKind.PrimaryPressed => Pressed(r.Accent.Mixed(r.Canvas, 0.2), radius, [Edge.Line(r.Ink, 1, 0.35)]),
                    SurfaceKind.ControlPressed => new(r.Raised, 1, radius, [Edge.Line(r.Line, 1, lineOpacity)], r.Ink, false, 0),
                    SurfaceKind.Danger => new(r.Surface, 1, radius, [Edge.Line(r.Line, 1, lineOpacity)], r.Danger, false, 0),
                    _ => new(r.Surface, 1, radius, [Edge.Line(r.Line, 1, lineOpacity)], r.Ink, false, 0),
                };
            default:
                // Cristal: calm surfaces, one filled primary per view, the selection in accentSoft.
                return kind switch
                {
                    SurfaceKind.Panel => new(r.Surface, 0.92, radius, [Edge.Line(r.Line, 1, 0.35)], r.Ink, false, 0),
                    SurfaceKind.Group => new(r.Surface, 0.6, radius, [Edge.Line(r.Line, 1, 0.3)], r.Ink, false, 0),
                    SurfaceKind.Well => new(r.Raised, 1, radius, [Edge.Line(r.Line, 1, 0.35)], r.Ink, false, 0),
                    SurfaceKind.Primary => new(r.Accent, 1, radius, [], r.OnAccent, true, 0),
                    SurfaceKind.PrimaryPressed => Pressed(r.Accent.Mixed(r.Ink, 0.15), radius, []),
                    SurfaceKind.ControlPressed => new(r.Raised.Mixed(r.Ink, 0.08), 1, radius, [Edge.Line(r.Line, 1, lineOpacity * 0.6)], r.Ink, false, 0),
                    SurfaceKind.PillSelected => new(r.AccentSoft, 1, radius, [], r.Ink, true, 0),
                    SurfaceKind.Pill => new(r.Surface, 0, radius, [], r.Ink, false, 0),
                    SurfaceKind.Danger => new(r.Surface, 1, radius, [Edge.Line(r.Line, 1, lineOpacity * 0.6)], r.Danger, false, 0),
                    _ => new(r.Surface, 1, radius, [Edge.Line(r.Line, 1, lineOpacity * 0.6)], r.Ink, false, 0),
                };
        }
    }
}
