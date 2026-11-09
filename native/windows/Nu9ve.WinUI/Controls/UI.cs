using Rectangle = Microsoft.UI.Xaml.Shapes.Rectangle;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Nu9ve.Shell;
using Nu9ve.Theme;
using Nu9ve.WinUI;

namespace Nu9ve.WinUI.Controls;

public enum TextRole { Display, Heading, Body, Muted, Label, Mono }

/// <summary>
/// Small builders so every page uses the same roles, type and spacing (nu9ve-theme-v1 §6: a view picks the role,
/// the skin picks the family). Colors are the shared role brushes, so they follow the theme live.
/// </summary>
public static class UI
{
    static ThemeService Theme => ThemeService.Current;

    public static TextBlock Text(string text, TextRole role = TextRole.Body, double? size = null)
    {
        var block = new TextBlock { Text = text, TextWrapping = TextWrapping.Wrap, Foreground = Theme.Brush(role is TextRole.Muted or TextRole.Label ? "muted" : "ink") };
        switch (role)
        {
            case TextRole.Display:
                block.FontFamily = Theme.DisplayFont; block.FontSize = size ?? 28; block.FontWeight = FontWeights.SemiBold;
                AutomationProperties.SetHeadingLevel(block, AutomationHeadingLevel.Level1);
                break;
            case TextRole.Heading:
                block.FontFamily = Theme.DisplayFont; block.FontSize = size ?? 17; block.FontWeight = FontWeights.SemiBold;
                AutomationProperties.SetHeadingLevel(block, AutomationHeadingLevel.Level2);
                break;
            case TextRole.Label:
                block.FontFamily = Theme.UiFont; block.FontSize = size ?? 11; block.FontWeight = FontWeights.SemiBold;
                break;
            case TextRole.Mono:
                block.FontFamily = new FontFamily("Cascadia Mono, Consolas"); block.FontSize = size ?? 12; block.IsTextSelectionEnabled = true;
                break;
            default:
                block.FontFamily = Theme.UiFont; block.FontSize = size ?? 14;
                break;
        }
        return block;
    }

    public static TextBlock Colored(string text, string role, double size = 13, bool strong = false) => new()
    {
        Text = text, Foreground = Theme.Brush(role), FontSize = size, TextWrapping = TextWrapping.Wrap,
        FontWeight = strong ? FontWeights.SemiBold : FontWeights.Normal, FontFamily = Theme.UiFont,
    };

    /// <summary>Segoe Fluent Icons glyph (the system's icon font, like SF Symbols on Mac).</summary>
    public static FontIcon Icon(string glyph, double size = 16, string role = "ink")
    {
        var icon = new FontIcon { Glyph = glyph, FontSize = size, Foreground = Theme.Brush(role), FontFamily = new FontFamily("Segoe Fluent Icons, Segoe MDL2 Assets") };
        AutomationProperties.SetAccessibilityView(icon, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        return icon;
    }

    public static StackPanel Row(double spacing, params UIElement?[] children) => Stack(Orientation.Horizontal, spacing, children);
    public static StackPanel Column(double spacing, params UIElement?[] children) => Stack(Orientation.Vertical, spacing, children);

    static StackPanel Stack(Orientation orientation, double spacing, UIElement?[] children)
    {
        var panel = new StackPanel { Orientation = orientation, Spacing = spacing };
        foreach (var child in children) if (child is not null) panel.Children.Add(child);
        if (orientation == Orientation.Horizontal)
            foreach (var child in panel.Children.OfType<FrameworkElement>()) child.VerticalAlignment = VerticalAlignment.Center;
        return panel;
    }

    public static NuSurface Surface(SurfaceKind kind, UIElement child, double padding = 16) =>
        new() { Kind = kind, Child = child, Padding = new Thickness(padding, padding * 0.75, padding, padding * 0.75) };

    /// <summary>A labelled section: heading then content, like the Mac app's grouped sections.</summary>
    public static StackPanel Section(string title, params UIElement?[] content)
    {
        var column = Column(8, [Text(title, TextRole.Heading), .. content]);
        column.Margin = new Thickness(0, 12, 0, 0);
        return column;
    }

    /// <summary>A button whose name, tooltip and accelerator text come from the shortcut catalog.</summary>
    public static NuButton Button(string text, Action? onClick = null, bool prominent = false, string? glyph = null, string? shortcut = null, bool destructive = false)
    {
        var content = glyph is null ? (UIElement)new TextBlock { Text = text } : Row(6, Icon(glyph, 14, "ink"), new TextBlock { Text = text });
        // The icon follows the button's own foreground (onAccent on a primary).
        if (content is StackPanel row && row.Children[0] is FontIcon icon) icon.ClearValue(FontIcon.ForegroundProperty);
        var button = new NuButton { Content = content, Prominent = prominent, Destructive = destructive };
        AutomationProperties.SetName(button, text);
        Describe(button, text, shortcut);
        if (onClick is not null) button.Click += (_, _) => onClick();
        return button;
    }

    public static NuToggle Toggle(string text, bool selected, Action onSelect, string? glyph = null, string? shortcut = null)
    {
        UIElement content = glyph is null ? new TextBlock { Text = text } : Row(6, Icon(glyph, 14), new TextBlock { Text = text });
        if (content is StackPanel row && row.Children[0] is FontIcon icon) icon.ClearValue(FontIcon.ForegroundProperty);
        var toggle = new NuToggle { Content = content, IsChecked = selected };
        AutomationProperties.SetName(toggle, text);
        Describe(toggle, text, shortcut);
        toggle.Click += (_, _) => onSelect();
        return toggle;
    }

    /// <summary>The app's catalog (nu9ve-interaction-v1 §1). Set once at start: `UI.Shortcuts = MyCatalog.Get`.</summary>
    public static Func<string, Shortcut?> Shortcuts { get; set; } = _ => null;

    /// <summary>Tooltip «Title (Ctrl+K)» from the catalog: no key written by hand in a view.</summary>
    public static void Describe(FrameworkElement element, string text, string? shortcutId)
    {
        if (shortcutId is null || Shortcuts(shortcutId) is not { } shortcut) { ToolTipService.SetToolTip(element, text); return; }
        var keys = shortcut.Keys?.Label;
        ToolTipService.SetToolTip(element, keys is null ? shortcut.Title : $"{shortcut.Title} ({keys})");
        if (keys is not null) AutomationProperties.SetAcceleratorKey(element, keys);
        AutomationProperties.SetHelpText(element, shortcut.Help);
    }

    /// <summary>A small keycap, drawn by the skin like any control (interaction §3).</summary>
    public static NuSurface Keycap(string key)
    {
        var text = new TextBlock { Text = key, FontSize = 11, FontFamily = new FontFamily("Cascadia Mono, Consolas"), Foreground = Theme.Brush("ink") };
        var cap = new NuSurface { Kind = SurfaceKind.Well, Child = text, Padding = new Thickness(6, 1, 6, 1), MinWidth = 22 };
        AutomationProperties.SetAccessibilityView(cap, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        return cap;
    }

    /// <summary>Usage bar: used / capacity, with the value spoken by Narrator.</summary>
    public static ProgressBar Meter(double fraction, string label, string role = "accent")
    {
        var bar = new ProgressBar
        {
            Minimum = 0, Maximum = 1, Value = Math.Clamp(fraction, 0, 1), Height = 6, MinHeight = 6,
            Foreground = Theme.Brush(role), Background = Theme.Brush("raised"), CornerRadius = new CornerRadius(Theme.Roles.Skin.ControlRadius()),
        };
        AutomationProperties.SetName(bar, label);
        return bar;
    }

    /// <summary>A badge (project, state). Text in ink on accentSoft keeps 7:1 (theme §2).</summary>
    public static NuSurface Badge(string text, SurfaceKind kind = SurfaceKind.PillSelected) =>
        new() { Kind = kind, Padding = new Thickness(8, 2, 8, 2), Child = Text(text, TextRole.Label, 12), HorizontalAlignment = HorizontalAlignment.Left };

    public static Grid Columns(params (UIElement Element, GridLength Width)[] cells)
    {
        var grid = new Grid { ColumnSpacing = 12 };
        for (var i = 0; i < cells.Length; i++)
        {
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = cells[i].Width });
            Grid.SetColumn((FrameworkElement)cells[i].Element, i);
            grid.Children.Add(cells[i].Element);
        }
        return grid;
    }

    public static Rectangle Rule() => new() { Height = 1, Fill = Theme.Brush("line"), Margin = new Thickness(0, 4, 0, 4) };

    /// <summary>Focusable wrapper that runs <paramref name="action"/> on Enter/Space, for whole-card selection.</summary>
    public static void Activate(UIElement element, Action action)
    {
        element.Tapped += (_, _) => action();
        element.KeyDown += (_, e) =>
        {
            if (e.Key is Windows.System.VirtualKey.Enter or Windows.System.VirtualKey.Space) { e.Handled = true; action(); }
        };
    }
}
