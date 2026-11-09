namespace Nu9ve.Shell;

public enum ShortcutScope { Global, View, Panel, Focus }

[Flags]
public enum Modifiers { None = 0, Ctrl = 1, Shift = 2, Alt = 4 }

/// <summary>A key in Windows terms. `Key` is a stable name (`N`, `1`, `Comma`, `Slash`, `F1`, `Enter`, `Backslash`).</summary>
public readonly record struct KeyChord(Modifiers Modifiers, string Key)
{
    /// <summary>What the keycap and tooltips print: «Ctrl+Shift+N».</summary>
    public string Label
    {
        get
        {
            var parts = new List<string>();
            if (Modifiers.HasFlag(Modifiers.Ctrl)) parts.Add("Ctrl");
            if (Modifiers.HasFlag(Modifiers.Shift)) parts.Add("Shift");
            if (Modifiers.HasFlag(Modifiers.Alt)) parts.Add("Alt");
            parts.Add(Key switch
            {
                "Comma" => ",", "Slash" => "/", "Backslash" => "\\", "Enter" => "Enter",
                "BracketLeft" => "[", "BracketRight" => "]", _ => Key,
            });
            return string.Join('+', parts);
        }
    }
}

/// <summary>One action of the app, declared once (nu9ve-interaction-v1 §1).</summary>
public sealed record Shortcut(string Id, string Title, string TitleEn, string Group, KeyChord? Keys, ShortcutScope Scope, string Keycap, string Help)
{
    public string Label(bool english) => english ? TitleEn : Title;
}

/// <summary>Rules every app's catalog must pass (nu9ve-interaction-v1 §1, windows-app-v1 §4).</summary>
public static class ShortcutRules
{
    /// <summary>Keys Windows or the shell own: a catalog may not take them.</summary>
    public static readonly IReadOnlyList<KeyChord> Reserved =
    [
        new(Modifiers.Alt, "F4"), new(Modifiers.Alt, "Tab"), new(Modifiers.Ctrl, "Escape"),
        new(Modifiers.Ctrl | Modifiers.Alt, "Delete"), new(Modifiers.None, "F10"), new(Modifiers.Shift, "F10"),
        new(Modifiers.Ctrl, "C"), new(Modifiers.Ctrl, "V"), new(Modifiers.Ctrl, "X"), new(Modifiers.Ctrl, "A"), new(Modifiers.Ctrl, "Z"),
    ];

    /// <summary>Two actions that can be active at once on the same chord.</summary>
    public static IEnumerable<(Shortcut A, Shortcut B)> Conflicts(IReadOnlyList<Shortcut> catalog) =>
        from a in catalog
        from b in catalog
        where string.CompareOrdinal(a.Id, b.Id) < 0 && a.Keys is not null && a.Keys == b.Keys
              && (a.Scope == b.Scope || a.Scope == ShortcutScope.Global || b.Scope == ShortcutScope.Global)
        select (a, b);
}
