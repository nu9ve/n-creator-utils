using Nu9ve.WinUI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Nu9ve.Shell;
using Nuun.Core.Shell;
using Nu9ve.Theme;
using Nu9ve.WinUI.Controls;
using Nuun.Core.Engine;

namespace Nuun.Pages;

/// <summary>
/// Ingreso (ingest-flow-v1): Entrada → Flujo → Resultado. Source and content together; destinations and the
/// plan together; one primary button authorizes the plan shown and its verified cleanup. The view only shows
/// what the engine planned: the client never recalculates plans or cleanup.
/// </summary>
public sealed class IngestView
{
    readonly AppState state;
    readonly MainWindow shell;
    readonly Grid host = new();
    // Result view parts updated in place while the engine reports progress.
    TextBlock? verb, file, numbers;
    ProgressBar? bar;
    TextBlock? log;
    TransferAnimation? animation;
    StackPanel? doneActions;

    public UIElement Root => host;

    public IngestView(AppState state, MainWindow shell)
    {
        this.state = state;
        this.shell = shell;
        Rebuild();
    }

    public void SetActive(bool active)
    {
        // The animation pauses while the window is inactive, like the Mac app.
        if (animation is not null) animation.Running = active && state.Busy;
    }

    public bool Run(string id)
    {
        switch (id)
        {
            case "ingest.back": state.Go(state.Step == IngestStep.Result ? IngestStep.Flow : IngestStep.Entry); return true;
            case "ingest.next": state.Go(state.Step == IngestStep.Entry ? IngestStep.Flow : IngestStep.Result); return true;
            case "ingest.review": _ = state.ReviewAsync(); return true;
            case "ingest.apply": if (state.Step == IngestStep.Flow) _ = state.ApplyAsync(); return true;
            case "ingest.eject": state.Report("Expulsar no está disponible en la maqueta: Windows aún no tiene el adaptador de volúmenes"); return true;
            default: return false;
        }
    }

    public void Rebuild()
    {
        verb = file = numbers = log = null; bar = null; animation = null; doneActions = null;
        var column = UI.Column(12, Steps());
        var (content, footer) = state.Step switch
        {
            IngestStep.Flow => Flow(),
            IngestStep.Result => Result(),
            _ => Entry(),
        };
        column.Children.Add(content);
        host.Children.Clear();
        host.Children.Add(PageScaffold.Build(column, footer, 860));
        UpdateProgress();
    }

    UIElement Steps()
    {
        var steps = new (IngestStep Step, string Label, string Glyph)[]
        {
            (IngestStep.Entry, "Entrada", ""), (IngestStep.Flow, "Flujo", ""), (IngestStep.Result, "Resultado", ""),
        };
        var row = UI.Row(2);
        for (var i = 0; i < steps.Length; i++)
        {
            var step = steps[i];
            var toggle = UI.Toggle(step.Label, state.Step == step.Step, () => state.Go(step.Step), step.Glyph);
            toggle.IsEnabled = step.Step switch { IngestStep.Flow => state.Source is not null && !state.Busy, IngestStep.Result => state.Progress is not null, _ => !state.Busy };
            AutomationProperties.SetName(toggle, $"Paso {i + 1} de 3: {step.Label}");
            row.Children.Add(toggle);
            if (i < steps.Length - 1) row.Children.Add(UI.Icon("", 10, "muted"));
        }
        var bar = UI.Surface(SurfaceKind.Group, row, 4);
        bar.HorizontalAlignment = HorizontalAlignment.Center;
        bar.Margin = new Thickness(0, 4, 0, 4);
        return bar;
    }

    // ---------- Entrada ----------

    (UIElement, UIElement) Entry()
    {
        var column = UI.Column(10);
        var sources = PageScaffold.Cards(260);
        foreach (var card in state.Snapshot.Cards)
        {
            var selected = state.Source?.Path == card.Path;
            var content = UI.Column(6,
                UI.Row(8, UI.Icon("", 18, selected ? "accentText" : "muted"), UI.Text(card.Name, TextRole.Body)),
                UI.Text($"{HomeView.FamilyLabel(card.Family)} · {Presentation.Bytes(card.UsedBytes)}", TextRole.Muted, 12),
                UI.Colored(Presentation.Recommendation(card.Recommendation), selected ? "accentText" : "muted", 12));
            var toggle = new NuToggle { Card = true, Content = content, IsChecked = selected, HorizontalContentAlignment = HorizontalAlignment.Left, Padding = new Thickness(12, 10, 12, 10), Margin = new Thickness(0, 0, 8, 8) };
            AutomationProperties.SetName(toggle, $"Fuente {card.Name}, {HomeView.FamilyLabel(card.Family)}");
            toggle.Click += (_, _) => state.ChooseSource(card);
            sources.Children.Add(toggle);
        }
        var other = UI.Button("Otra carpeta…", () => state.Report("Maqueta: el selector de carpeta del sistema llega con el adaptador de Windows"), glyph: "");
        column.Children.Add(UI.Section("Fuente", sources, other));

        var project = new TextBox { Text = state.Project, PlaceholderText = "Proyecto del lote", MaxWidth = 360, HorizontalAlignment = HorizontalAlignment.Left };
        AutomationProperties.SetName(project, "Proyecto del lote");
        project.TextChanged += (_, _) => state.Project = project.Text;
        var recent = UI.Row(6);
        foreach (var p in state.Snapshot.Projects.Where(p => p.Stage != "finished").Take(3))
            recent.Children.Add(UI.Toggle(p.Name, p.Name == state.Project, () => { state.Project = p.Name; project.Text = p.Name; }));
        column.Children.Add(UI.Section("Proyecto", project, recent,
            UI.Text("El proyecto del lote rellena sólo las unidades sin proyecto propio.", TextRole.Muted, 12)));

        if (state.Source is { } source)
        {
            var summary = UI.Surface(SurfaceKind.Panel, UI.Column(6,
                UI.Text("Contenido", TextRole.Heading, 15),
                UI.Text(source.Family == "sony" ? "1 clip · datos de cámara (índices, miniaturas)" : "2 fotos · registro de vuelo", TextRole.Body),
                UI.Text("Elegir una fuente no recorre los archivos; el plan lo hace al revisar.", TextRole.Muted, 12)));
            column.Children.Add(summary);
        }
        var footer = PageScaffold.Footer(
            state.Source is null ? "Elige una fuente para continuar." : $"{state.Source.Name} → {state.Project}. Revisar no copia nada.",
            UI.Button("Revisar plan", () => _ = state.ReviewAsync(), prominent: true, glyph: "", shortcut: "ingest.review"));
        return (column, footer);
    }

    // ---------- Flujo ----------

    (UIElement, UIElement) Flow()
    {
        var plan = state.Plan!;
        var column = UI.Column(10);
        var destinations = new Expander
        {
            Header = UI.Text("Destinos y opciones"), HorizontalAlignment = HorizontalAlignment.Stretch, HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Background = ThemeServiceBrush("surface"),
            Content = UI.Column(8,
                Destination("MEDIA-DEMO", "Respaldo (archivo) · obligatorio", true),
                Destination("ESSD-DEMO", "Copia para editar · verificada antes de limpiar", true),
                UI.Text("Limpieza: después de verificar todos los destinos requeridos, según la política del plan.", TextRole.Muted, 12)),
        };
        column.Children.Add(destinations);

        var items = UI.Column(6);
        items.Children.Add(UI.Row(8, UI.Icon("", 18), UI.Text($"Plan revisable · {plan.Files} archivos · {Presentation.Bytes(plan.Bytes)}", TextRole.Heading)));
        items.Children.Add(UI.Badge(plan.Project));
        foreach (var step in plan.Steps) items.Children.Add(UI.Row(8, UI.Icon("", 14, "muted"), UI.Text(step, TextRole.Body, 13)));
        if (plan.Warning is { } warning)
            items.Children.Add(UI.Row(8, UI.Icon("", 14, "warning"), UI.Colored(warning, "warning", 13)));
        items.Children.Add(UI.Rule());
        foreach (var item in plan.Items)
        {
            var targets = string.Join("  ", item.Destinations.Select(d => d == PlanDestination.Archive ? "MEDIA" : "SSD"));
            items.Children.Add(UI.Columns(
                (UI.Column(2,
                    UI.Row(8, UI.Icon(item.Support ? "" : "", 15), UI.Text(item.Name, TextRole.Body)),
                    UI.Text($"{item.Project} · {targets} · {item.Files} archivo{(item.Files == 1 ? "" : "s")}", TextRole.Muted, 12),
                    UI.Text(item.Note, TextRole.Muted, 12)), new GridLength(1, GridUnitType.Star)),
                (UI.Text(Presentation.Bytes(item.Bytes), TextRole.Mono), GridLength.Auto)));
            items.Children.Add(UI.Rule());
        }
        column.Children.Add(UI.Surface(SurfaceKind.Panel, items));

        // One button: it names what it authorizes (backup and, if the plan says so, verified release).
        var primary = plan.ReleasesSource ? "Respaldar y liberar origen" : "Respaldar";
        var footer = PageScaffold.Footer("Plan listo para revisar; no se ha copiado nada.",
            UI.Button("Atrás", () => state.Go(IngestStep.Entry), shortcut: "ingest.back"),
            UI.Button(primary, () => _ = state.ApplyAsync(), prominent: true, glyph: "", shortcut: "ingest.apply"));
        return (column, footer);
    }

    static UIElement Destination(string name, string role, bool online) =>
        UI.Row(10, UI.Icon(online ? "" : "", 14, online ? "success" : "danger"), UI.Column(0, UI.Text(name), UI.Text(role, TextRole.Muted, 12)));

    static Microsoft.UI.Xaml.Media.Brush ThemeServiceBrush(string role) => ThemeService.Current.Brush(role);

    // ---------- Resultado (nu9ve-interaction-v1 §7 «Acciones largas») ----------

    (UIElement, UIElement?) Result()
    {
        verb = UI.Text("", TextRole.Heading, 20);
        verb.HorizontalAlignment = HorizontalAlignment.Center;
        file = UI.Text("", TextRole.Muted, 13);
        file.HorizontalAlignment = HorizontalAlignment.Center;
        animation = new TransferAnimation { Width = 320, HorizontalAlignment = HorizontalAlignment.Center };
        bar = UI.Meter(0, "Progreso total", "info");
        bar.Height = 8;
        numbers = UI.Text("", TextRole.Mono);
        numbers.HorizontalAlignment = HorizontalAlignment.Center;
        log = UI.Text("", TextRole.Mono);
        var detail = new Expander
        {
            Header = UI.Text("Detalle"), IsExpanded = false, HorizontalAlignment = HorizontalAlignment.Stretch, HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Background = ThemeServiceBrush("surface"), Content = log,
        };
        var pause = UI.Text("Cancelar conserva lo ya copiado y verificado; nada se limpia sin verificar.", TextRole.Muted, 12);
        pause.HorizontalAlignment = HorizontalAlignment.Center;
        doneActions = UI.Row(8,
            UI.Button("Nueva ingesta", () => state.NewIngest(), prominent: true, shortcut: "ingest.new"),
            UI.Button("Ver biblioteca", () => shell.Navigate("library")));
        doneActions.HorizontalAlignment = HorizontalAlignment.Center;
        var column = UI.Column(14, verb, file, animation, bar, numbers, pause, doneActions, detail);
        column.Margin = new Thickness(0, 24, 0, 0);
        var footer = state.Busy ? PageScaffold.Footer("Puedes navegar mientras tanto; sólo esta ingesta queda bloqueada.",
            UI.Button("Cancelar", state.Cancel, destructive: true)) : null;
        return (column, footer);
    }

    public void UpdateProgress()
    {
        if (state.Progress is not { } progress || verb is null) return;
        verb.Text = progress.Finished ? "Listo: respaldo verificado" : $"{progress.Verb}";
        file!.Text = progress.CurrentFile is null ? $"{progress.FilesDone} de {progress.FilesTotal} archivos" : $"{progress.CurrentFile}  ·  {progress.From} → {progress.To}";
        bar!.Value = progress.Fraction;
        AutomationProperties.SetName(bar, $"Progreso total {progress.Fraction:P0}");
        numbers!.Text = $"{progress.Fraction:P0} · {Presentation.Bytes(progress.OverallDone)} de {Presentation.Bytes(progress.OverallTotal)} copiados y verificados · {progress.FilesDone}/{progress.FilesTotal} archivos";
        log!.Text = progress.Log.Count == 0 ? "Esperando el primer paso…" : string.Join("\n", progress.Log);
        animation!.Running = !progress.Finished && state.Busy;
        animation.Visibility = progress.Finished ? Visibility.Collapsed : Visibility.Visible;
        doneActions!.Visibility = progress.Finished ? Visibility.Visible : Visibility.Collapsed;
    }
}
