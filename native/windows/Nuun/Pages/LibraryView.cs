using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Nu9ve.Shell;
using Nuun.Core.Shell;
using Nu9ve.Theme;
using Nu9ve.WinUI.Controls;
using Nu9ve.WinUI;

namespace Nuun.Pages;

/// <summary>Biblioteca: Proyectos, Discos (map of what fills each disk) and Actividad. Read only in the maqueta.</summary>
public static class LibraryView
{
    static string tab = "projects";

    public static UIElement Build(AppState state, MainWindow shell)
    {
        var column = UI.Column(12, UI.Text("Biblioteca", TextRole.Display));
        var tabs = UI.Row(4);
        foreach (var (id, label) in new[] { ("projects", "Proyectos"), ("disks", "Discos"), ("activity", "Actividad") })
            tabs.Children.Add(UI.Toggle(label, tab == id, () => { tab = id; shell.Navigate("library"); }));
        column.Children.Add(tabs);
        column.Children.Add(tab switch { "disks" => Disks(state), "activity" => Activity(state), _ => Projects(state) });
        return PageScaffold.Build(column);
    }

    static UIElement Projects(AppState state)
    {
        var list = UI.Column(6);
        foreach (var project in state.Snapshot.Projects)
        {
            list.Children.Add(UI.Surface(SurfaceKind.Panel, UI.Columns(
                (UI.Row(10, UI.Icon("", 16, "accentText"), UI.Text(project.Name)), new GridLength(1, GridUnitType.Star)),
                (UI.Text($"{project.Units} unidades", TextRole.Muted, 12), GridLength.Auto),
                (UI.Badge(Presentation.Stage(project.Stage), project.Stage == "finished" ? SurfaceKind.Well : SurfaceKind.PillSelected), GridLength.Auto)), 12));
        }
        return list;
    }

    /// <summary>
    /// What the Rust engine read from this PC, apart from the maqueta. Read only: Windows cannot back up yet, so
    /// these cards never enter the maqueta's ingest (a simulated «backup» of a real card would mislead).
    /// </summary>
    static UIElement? Live(AppState state)
    {
        if (state.LiveVolumes is null && state.LiveCards is null) return null;
        var column = UI.Column(8,
            UI.Text("Leído por el motor de Rust en este equipo · sólo lectura", TextRole.Heading, 15),
            UI.Text("Todavía no se puede respaldar desde Windows: estas tarjetas no entran al Ingreso de la maqueta.", TextRole.Muted, 12));
        var volumes = state.LiveVolumes ?? [];
        column.Children.Add(UI.Text(volumes.Count == 0
            ? "Destinos registrados: ninguno. Registrar MEDIA o ESSD en Windows llega con el siguiente paso del motor."
            : $"Destinos registrados: {string.Join(", ", volumes.Select(v => $"{v.Name} ({(v.Online ? "en línea" : "desconectado")})"))}", TextRole.Body, 13));
        var cards = state.LiveCards ?? [];
        if (cards.Count == 0)
            column.Children.Add(UI.Text("Tarjetas: ninguna montada (unidades extraíbles y externas, nunca C: ni un destino).", TextRole.Muted, 12));
        foreach (var card in cards)
        {
            var detail = card.HasHint
                ? $"{HomeView.FamilyLabel(card.Family)} · {Presentation.Recommendation(card.Recommendation)}{(card.LastProject is { } p ? $" · último proyecto: {p}" : "")}"
                : "Sin leer su contenido: ningún destino registrado está en línea (regla del producto).";
            column.Children.Add(UI.Surface(SurfaceKind.Well, UI.Row(10, UI.Icon("", 16, "accentText"),
                UI.Column(0, UI.Text($"{card.Name} · {card.Path}"), UI.Text(detail, TextRole.Muted, 12))), 10));
        }
        return UI.Surface(SurfaceKind.Panel, column);
    }

    static UIElement Disks(AppState state)
    {
        var list = UI.Column(12);
        if (Live(state) is { } live) list.Children.Add(live);
        foreach (var volume in state.Snapshot.Volumes)
        {
            list.Children.Add(HomeView.Disk(volume));
            if (!volume.Online || volume.Capacity is not { } capacity) continue;
            // Fixture shares of the used space; the real split comes from the engine's storage map.
            var used = capacity - (volume.Free ?? 0);
            var parts = volume.Role == Core.Engine.VolumeRole.Archive
                ? new[] { ("001_raw", 0.72, "accent"), ("002_finales", 0.16, "info"), ("003_curado", 0.08, "success"), ("004_datos", 0.04, "warning") }
                : new[] { ("001_proyectos", 0.81, "accent"), ("002_finales", 0.19, "info") };
            list.Children.Add(Map(parts.Select(p => (p.Item1, used * p.Item2, p.Item3)).Append(("libre", (double)(volume.Free ?? 0), "raised")).ToArray()));
        }
        return list;
    }

    /// <summary>Squarified treemap (Nu9ve.Shell.Treemap, ported from Swift). Tiles carry their name and size for Narrator.</summary>
    static UIElement Map((string Name, double Bytes, string Role)[] parts)
    {
        var canvas = new Canvas { Height = 180 };
        void Draw(double width)
        {
            canvas.Children.Clear();
            var rects = Treemap.Layout(parts.Select(p => p.Bytes).ToArray(), new Treemap.Rect(0, 0, width, 180));
            for (var i = 0; i < parts.Length; i++)
            {
                var r = rects[i];
                if (r.Width < 1 || r.Height < 1) continue;
                var tile = new Border
                {
                    Width = Math.Max(0, r.Width - 2), Height = Math.Max(0, r.Height - 2),
                    Background = ThemeService.Current.Brush(parts[i].Role),
                    CornerRadius = new CornerRadius(Math.Min(4, ThemeService.Current.Roles.Skin.ControlRadius())),
                    Padding = new Thickness(6, 4, 6, 4),
                };
                if (r.Width > 70 && r.Height > 34)
                {
                    // Black or white, whichever reads on this tile's fill (theme §3 step 5).
                    var fill = ThemeService.Current.Roles.ByName()[parts[i].Role];
                    var ink = ThemeService.NewBrush(fill.OnColor);
                    tile.Child = UI.Column(0,
                        new TextBlock { Text = parts[i].Name, Foreground = ink, FontSize = 12, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold },
                        new TextBlock { Text = Presentation.Bytes((long)parts[i].Bytes), Foreground = ink, FontSize = 11 });
                }
                AutomationProperties.SetName(tile, $"{parts[i].Name}: {Presentation.Bytes((long)parts[i].Bytes)}");
                Canvas.SetLeft(tile, r.X);
                Canvas.SetTop(tile, r.Y);
                canvas.Children.Add(tile);
            }
        }
        canvas.SizeChanged += (_, e) => Draw(e.NewSize.Width);
        return canvas;
    }

    static UIElement Activity(AppState state)
    {
        var list = UI.Column(6);
        foreach (var job in state.Snapshot.Jobs)
            list.Children.Add(UI.Surface(SurfaceKind.Panel, UI.Columns(
                (UI.Row(10, UI.Icon(job.NeedsAttention ? "" : "", 16, job.NeedsAttention ? "warning" : "success"),
                    UI.Column(0, UI.Text(job.Title), UI.Text(Presentation.JobState(job.State), TextRole.Muted, 12))), new GridLength(1, GridUnitType.Star)),
                (UI.Text(job.Id, TextRole.Mono), GridLength.Auto)), 12));
        return list;
    }
}
