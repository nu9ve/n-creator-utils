using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Nu9ve.Shell;
using Nuun.Core.Shell;
using Nu9ve.Theme;
using Nu9ve.WinUI.Controls;

namespace Nuun.Pages;

/// <summary>
/// A page: a reading column aligned left with the bar (the Windows Settings pattern, origin nove MainWindow
/// ReadableColumn), gutter 16/24/32 by width, and an optional fixed footer for the view's actions — the Mac
/// app's floating action foot. The footer holds at most one prominent button.
/// </summary>
public static class PageScaffold
{
    public static Grid Build(UIElement content, UIElement? footer = null, double maxWidth = 980)
    {
        var grid = new Grid();
        grid.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        var column = new Border { Child = content, MaxWidth = maxWidth, HorizontalAlignment = HorizontalAlignment.Left };
        var scroll = new ScrollViewer { Content = column, HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled, VerticalScrollBarVisibility = ScrollBarVisibility.Auto };
        grid.Children.Add(scroll);
        Border? foot = null;
        if (footer is not null)
        {
            foot = new Border { Child = UI.Surface(SurfaceKind.Panel, footer, 12) };
            Grid.SetRow(foot, 1);
            grid.Children.Add(foot);
        }
        grid.SizeChanged += (_, e) =>
        {
            var gutter = WindowLayout.Gutter(e.NewSize.Width);
            column.Padding = new Thickness(gutter, 8, gutter, 24);
            column.Width = Math.Min(maxWidth, Math.Max(0, e.NewSize.Width));
            if (foot is not null) foot.Padding = new Thickness(gutter * 0.5, 0, gutter * 0.5, gutter * 0.5);
        };
        return grid;
    }

    /// <summary>Footer layout: a status line that says what will (not) happen, then actions on the right.</summary>
    public static Grid Footer(string status, params UIElement[] buttons)
    {
        var line = UI.Text(status, TextRole.Muted, 12);
        line.VerticalAlignment = VerticalAlignment.Center;
        line.TextTrimming = TextTrimming.CharacterEllipsis;
        line.MaxLines = 2;
        return UI.Columns((line, new GridLength(1, GridUnitType.Star)), (UI.Row(8, buttons), GridLength.Auto));
    }

    /// <summary>A grid of cards that reflows by width (one column in compact).</summary>
    public static VariableSizedWrapGrid Cards(double itemWidth = 300) => new()
    {
        Orientation = Orientation.Horizontal, ItemWidth = itemWidth, HorizontalChildrenAlignment = HorizontalAlignment.Left,
    };
}
