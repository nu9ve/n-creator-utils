using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Nu9ve.Shell;
using Nuun.Core.Shell;
using Nu9ve.Theme;
using Nu9ve.WinUI.Controls;
using Nuun.Core.Engine;

namespace Nuun.Pages;

/// <summary>Inicio: what needs attention, connected cards, recent projects and disks. Read only.</summary>
public static class HomeView
{
    public static UIElement Build(AppState state, MainWindow shell)
    {
        var snapshot = state.Snapshot;
        var column = UI.Column(12);
        column.Children.Add(UI.Text("Inicio", TextRole.Display));
        column.Children.Add(UI.Text("Una multiherramienta para creadores: ingresa, respalda y prepara media para trabajar.", TextRole.Muted));
        if (!state.Engine.IsFixture)
        {
            // Real data from this PC: say plainly what Windows can and cannot do yet.
            var real = new NuSurface { Kind = SurfaceKind.Well, Padding = new Thickness(14, 10, 14, 10) };
            real.Child = UI.Row(10, UI.Icon(Glyph.Info, 16, "info"), UI.Colored("Datos reales de este equipo, leídos por el motor de Rust. Respaldar y liberar desde Windows todavía no está disponible.", "ink", 13));
            column.Children.Add(real);
        }

        var attention = snapshot.Jobs.Where(j => j.NeedsAttention).ToList();
        if (attention.Count > 0)
        {
            var list = UI.Column(8);
            foreach (var job in attention)
            {
                var banner = new NuSurface { Kind = SurfaceKind.Well, Padding = new Thickness(14, 10, 14, 10) };
                banner.Child = UI.Columns(
                    (UI.Row(10, UI.Icon("", 18, "warning"), UI.Column(2, UI.Colored(job.Title, "ink", 14, strong: true), UI.Colored(Presentation.JobState(job.State), "warning"))), new GridLength(1, GridUnitType.Star)),
                    (UI.Button("Descartar aviso", () => state.Report("Maqueta: el aviso se apaga, el trabajo conserva su estado")), GridLength.Auto));
                list.Children.Add(banner);
            }
            column.Children.Add(UI.Section("Requiere atención", list));
        }

        var cards = PageScaffold.Cards(320);
        foreach (var card in snapshot.Cards) cards.Children.Add(Card(card, state, shell));
        column.Children.Add(UI.Section("Tarjetas conectadas", snapshot.Cards.Count == 0
            ? UI.Text("Sin tarjetas. Con un destino registrado conectado, al insertar una SD se ofrece la vista compacta.", TextRole.Muted)
            : cards));

        var recent = new VariableSizedWrapGrid { Orientation = Orientation.Horizontal };
        foreach (var project in snapshot.Projects.Where(p => p.Stage != "finished"))
        {
            var pill = UI.Button(project.Name, () => { state.Project = project.Name; shell.Navigate("ingest"); }, glyph: "");
            pill.Margin = new Thickness(0, 0, 6, 6);
            ToolTipService.SetToolTip(pill, $"{Presentation.Stage(project.Stage)} · {project.Units} unidades. Úsalo en el ingreso");
            recent.Children.Add(pill);
        }
        column.Children.Add(UI.Section("Proyectos recientes", recent.Children.Count == 0
            ? UI.Text("Sin proyectos todavía: aparecen al ingresar media con un proyecto.", TextRole.Muted)
            : recent));

        var disks = UI.Column(8);
        foreach (var volume in snapshot.Volumes) disks.Children.Add(Disk(volume));
        column.Children.Add(UI.Section("Discos", disks.Children.Count == 0
            ? UI.Column(8, UI.Text("Ningún destino registrado en este equipo.", TextRole.Muted),
                UI.Button("Registrar destino en Biblioteca → Discos", () => shell.Navigate("library"), glyph: Glyph.Add))
            : disks));
        return PageScaffold.Build(column);
    }

    static UIElement Card(SourceCard card, AppState state, MainWindow shell)
    {
        var used = card.CapacityBytes > 0 ? (double)card.UsedBytes / card.CapacityBytes : 0;
        var content = UI.Column(8,
            UI.Row(10, UI.Icon("", 22, "accentText"), UI.Column(0, UI.Text(card.Name, TextRole.Heading, 15), UI.Text(card.Device ?? FamilyLabel(card.Family), TextRole.Muted, 12))),
            UI.Meter(used, $"Uso de {card.Name}: {Presentation.Bytes(card.UsedBytes)} de {Presentation.Bytes(card.CapacityBytes)}"),
            UI.Text($"{Presentation.Bytes(card.UsedBytes)} de {Presentation.Bytes(card.CapacityBytes)}", TextRole.Muted, 12),
            UI.Colored(Presentation.Recommendation(card.Recommendation), "accentText", 13),
            UI.Button("Preparar respaldo", () => { state.ChooseSource(card); shell.Navigate("ingest"); }, glyph: ""));
        var surface = UI.Surface(SurfaceKind.Panel, content);
        surface.Margin = new Thickness(0, 0, 12, 12);
        AutomationProperties.SetName(surface, $"Tarjeta {card.Name}");
        return surface;
    }

    public static UIElement Disk(Volume volume)
    {
        var fraction = volume.Capacity is > 0 && volume.Free is { } free ? 1 - (double)free / volume.Capacity.Value : 0;
        var role = volume.Role switch { VolumeRole.Archive => "Archivo", VolumeRole.Work => "Trabajo", _ => "Otro" };
        var state = !volume.Online ? "Desconectado"
            : volume.Capacity is null ? $"{Presentation.Bytes(volume.Free)} libres" : $"{Presentation.Bytes(volume.Free)} libres de {Presentation.Bytes(volume.Capacity)}";
        var row = UI.Columns(
            (UI.Row(10, UI.Icon("", 18, volume.Online ? "success" : "muted"), UI.Column(0, UI.Text(volume.Name, TextRole.Body), UI.Text($"{role} · {volume.Prefix}", TextRole.Muted, 12))), new GridLength(220)),
            (UI.Column(4, UI.Meter(fraction, $"{volume.Name}: {state}", volume.Online ? "accent" : "line"), UI.Text(state, TextRole.Muted, 12)), new GridLength(1, GridUnitType.Star)));
        return UI.Surface(SurfaceKind.Panel, row, 14);
    }

    public static string FamilyLabel(string family) => family switch
    {
        "sony" => "Sony", "dji" => "DJI", "stalker" => "Stalker", "h6" => "Zoom H6", "gopro" => "GoPro", "insta360" => "Insta360", "canon" => "Canon", _ => "Tarjeta",
    };
}

/// <summary>Segoe Fluent Icons code points by name, so a glyph never depends on how an editor writes an escape.</summary>
static class Glyph
{
    public static readonly string Info = char.ConvertFromUtf32(0xE946);
    public static readonly string Add = char.ConvertFromUtf32(0xE710);
}
