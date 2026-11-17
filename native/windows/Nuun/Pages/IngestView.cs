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
            case "ingest.eject": state.Report("Expulsar desde nüün en Windows llega en el paso 5 de la fase A; usa «Quitar hardware de forma segura»."); return true;
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
        if (state.Source is { Folder: true } folder)
        {
            var chosen = new NuToggle { Card = true, IsChecked = true, HorizontalContentAlignment = HorizontalAlignment.Left, Padding = new Thickness(12, 10, 12, 10), Margin = new Thickness(0, 0, 8, 8),
                Content = UI.Column(6, UI.Row(8, UI.Icon("\uE8B7", 18, "accentText"), UI.Text(folder.Name, TextRole.Body)),
                    UI.Text(folder.Path, TextRole.Muted, 12), UI.Colored("Carpeta: se respalda y se conserva", "accentText", 12)) };
            AutomationProperties.SetName(chosen, $"Fuente carpeta {folder.Name}");
            sources.Children.Add(chosen);
        }
        column.Children.Add(UI.Section("Fuente", sources, OtherFolder()));

        var project = new TextBox { Text = state.Project, PlaceholderText = "Proyecto del lote", MaxWidth = 360, HorizontalAlignment = HorizontalAlignment.Left };
        AutomationProperties.SetName(project, "Proyecto del lote");
        project.TextChanged += (_, _) => state.Project = project.Text;
        var recent = UI.Row(6);
        foreach (var p in state.Snapshot.Projects.Where(p => p.Stage != "finished").Take(3))
            recent.Children.Add(UI.Toggle(p.Name, p.Name == state.Project, () => { state.Project = p.Name; project.Text = p.Name; }));
        column.Children.Add(UI.Section("Proyecto", project, recent,
            UI.Text("El proyecto del lote rellena sólo las unidades sin proyecto propio.", TextRole.Muted, 12)));
        if (!state.Engine.IsFixture) column.Children.Add(Destinations());

        if (state.Source is { } source)
        {
            UIElement content;
            if (state.Engine.IsFixture)
            {
                content = UI.Column(6,
                    UI.Text(source.Family == "sony" ? "1 clip · datos de cámara (índices, miniaturas)" : "2 fotos · registro de vuelo", TextRole.Body),
                    UI.Text("Elegir una fuente no recorre los archivos; el plan lo hace al revisar.", TextRole.Muted, 12));
            }
            else if (state.Discovery is { } found)
            {
                // What the Rust engine read on this card (discover), not a maqueta.
                var kinds = string.Join(" · ", found.Categories.Select(c => $"{c.Value} {CategoryName(c.Key)}"));
                content = UI.Column(6,
                    UI.Text($"{found.Units} unidades · {Presentation.Bytes(found.Bytes)}", TextRole.Body),
                    UI.Text(kinds + (found.Camera.Length > 0 ? $" · cámara {found.Camera}" : ""), TextRole.Muted, 12),
                    found.Devices.Count > 0
                        ? UI.Text("Dispositivo (EXIF): " + string.Join(" · ", found.Devices), TextRole.Muted, 12)
                        : null,
                    found.ToConfirm > 0
                        ? UI.Colored($"{found.ToConfirm} unidades piden confirmar su dispositivo antes de clasificarlas.", "warning", 12)
                        : null,
                    found.NotPorted.Contains("mission")
                        ? UI.Text("Todavía sin cruzar las fotos con el registro de misiones del archivo.", TextRole.Muted, 12)
                        : null,
                    found.Skipped.Count > 0 ? UI.Colored($"{found.Skipped.Count} capturas activas se omiten.", "warning", 12) : null);
            }
            else
            {
                content = UI.Column(6,
                    UI.Text("Elegir una fuente no recorre los archivos.", TextRole.Muted, 12),
                    UI.Button("Leer contenido", () => _ = state.DiscoverAsync(), glyph: char.ConvertFromUtf32(0xE721)));
            }
            column.Children.Add(UI.Surface(SurfaceKind.Panel, UI.Column(6, UI.Text("Contenido", TextRole.Heading, 15), content)));
        }
        var footer = PageScaffold.Footer(
            state.Source is null ? "Elige una fuente para continuar." : $"{state.Source.Name} → {(state.Project.Trim().Length > 0 ? state.Project.Trim() : "proyecto general")}. Revisar no copia nada.",
            UI.Button("Revisar plan", () => _ = state.ReviewAsync(), prominent: true, glyph: "", shortcut: "ingest.review"));
        return (column, footer);
    }

    /// <summary>
    /// «Otra carpeta…»: the system picker, or a typed path (Narrator and UI Automation reach both). A folder is
    /// backed up and kept; only the root of a mounted card is ever released.
    /// </summary>
    UIElement OtherFolder()
    {
        if (state.Engine.IsFixture)
            return UI.Button("Otra carpeta…", () => state.Report("Maqueta: elegir una carpeta necesita el motor de Rust junto a la app"), glyph: "\uE8B7");
        var path = new TextBox { PlaceholderText = "D:\\fotos\\viaje", Width = 320, Text = state.Source is { Folder: true } f ? f.Path : "" };
        AutomationProperties.SetName(path, "Carpeta de origen");
        return UI.Row(8, path,
            UI.Button("Elegir…", () => _ = PickFolderAsync(path), glyph: "\uE8B7"),
            UI.Button("Usar carpeta", () => state.UseFolder(path.Text)));
    }

    async Task PickFolderAsync(TextBox target)
    {
        var picker = new Windows.Storage.Pickers.FolderPicker { SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.ComputerFolder };
        picker.FileTypeFilter.Add("*");
        // Unpackaged WinUI: the picker needs its owner window.
        WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(shell));
        if (await picker.PickSingleFolderAsync() is { } folder) { target.Text = folder.Path; state.UseFolder(folder.Path); }
    }

    /// <summary>
    /// Registered destinations online now, as the engine read them. Without a work disk the plan is archive-only, which
    /// the person confirms for this operation; it never becomes the default (ingest-flow-v1).
    /// </summary>
    UIElement Destinations()
    {
        var archives = state.OnlineDestinations(VolumeRole.Archive);
        var works = state.OnlineDestinations(VolumeRole.Work);
        if (archives.Count == 0)
            return UI.Section("Destinos", UI.Colored("No hay un destino de archivo en línea. Regístralo o conéctalo en Biblioteca → Discos.", "warning", 13),
                UI.Button("Ir a Discos", () => shell.Navigate("library"), glyph: "\uEDA2"));

        var archive = new ComboBox { MinWidth = 240 };
        foreach (var v in archives) archive.Items.Add(v.Name);
        archive.SelectedItem = state.Archive;
        AutomationProperties.SetName(archive, "Destino de archivo");
        archive.SelectionChanged += (_, _) => state.ChangePlanOptions(() => state.Archive = archive.SelectedItem as string);

        const string none = "Sin copia para editar";
        var work = new ComboBox { MinWidth = 240 };
        foreach (var v in works) work.Items.Add(v.Name);
        work.Items.Add(none);
        work.SelectedItem = state.Work ?? none;
        AutomationProperties.SetName(work, "Destino de trabajo");
        work.SelectionChanged += (_, _) => state.ChangePlanOptions(() => state.Work = work.SelectedItem is string w && w != none ? w : null);

        var column = UI.Column(8,
            UI.Row(10, UI.Text("Respaldo", TextRole.Body), archive),
            UI.Row(10, UI.Text("Para editar", TextRole.Body), work));
        if (state.Work is null)
        {
            var confirm = new CheckBox { Content = "Sólo respaldo en esta operación", IsChecked = state.ConfirmArchiveOnly };
            AutomationProperties.SetName(confirm, "Confirmar sólo respaldo en esta operación");
            // Checked/Unchecked: also raised by the keyboard, Narrator and UI Automation (Click is not).
            confirm.Checked += (_, _) => state.ChangePlanOptions(() => state.ConfirmArchiveOnly = true);
            confirm.Unchecked += (_, _) => state.ChangePlanOptions(() => state.ConfirmArchiveOnly = false);
            column.Children.Add(confirm);
            column.Children.Add(UI.Text(works.Count == 0 && state.LiveVolumes?.Any(v => v.Role == VolumeRole.Work) == true
                ? "La copia para editar queda pendiente: se entrega después desde el archivo."
                : "Sin disco de trabajo registrado: esta operación no hace copia para editar.", TextRole.Muted, 12));
        }
        var folder = state.Source?.Folder == true;
        var cleanup = new CheckBox { Content = "Liberar la tarjeta tras verificar todos los destinos", IsChecked = state.Cleanup && !folder, IsEnabled = !folder };
        AutomationProperties.SetName(cleanup, "Liberar la tarjeta tras verificar");
        cleanup.Checked += (_, _) => state.ChangePlanOptions(() => state.Cleanup = true);
        cleanup.Unchecked += (_, _) => state.ChangePlanOptions(() => state.Cleanup = false);
        column.Children.Add(cleanup);
        if (folder) column.Children.Add(UI.Text("Una carpeta se respalda y se conserva: sólo se libera la raíz de una tarjeta montada.", TextRole.Muted, 12));
        return UI.Section("Destinos", column);
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
            Content = state.Engine.IsFixture
                ? UI.Column(8,
                    Destination("MEDIA-DEMO", "Respaldo (archivo) · obligatorio", true),
                    Destination("ESSD-DEMO", "Copia para editar · verificada antes de limpiar", true),
                    UI.Text("Limpieza: después de verificar todos los destinos requeridos, según la política del plan.", TextRole.Muted, 12))
                : UI.Column(8,
                    Destination(state.Archive ?? "—", "Respaldo (archivo) · obligatorio", true),
                    state.Work is { } w
                        ? Destination(w, "Copia para editar · verificada antes de limpiar", true)
                        : UI.Text(plan.Steps.Count > 1 ? plan.Steps[1] : "Sin copia para editar", TextRole.Muted, 12),
                    UI.Text(plan.ReleasesSource
                        ? "Limpieza: después de verificar todos los destinos requeridos, según la política del plan."
                        : "El origen se conserva: este plan no limpia nada.", TextRole.Muted, 12),
                    UI.Text("Cambiar destinos u opciones invalida este plan: vuelve a Entrada.", TextRole.Muted, 12)),
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

    static string CategoryName(string code) => code switch
    {
        "video" => "video", "image" => "foto", "raw_image" => "RAW", "audio" => "audio",
        "stalker" => "sesión Stalker", "support" => "soporte de cámara", _ => code,
    };

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
        var pause = UI.Text("Pausar conserva lo ya copiado y verificado; nada se limpia sin verificar.", TextRole.Muted, 12);
        pause.HorizontalAlignment = HorizontalAlignment.Center;
        doneActions = UI.Row(8);
        if (state.PausedJob is { } paused)
            doneActions.Children.Add(UI.Button("Reanudar", () => _ = state.ResumeAsync(paused), prominent: true, glyph: "\uE768"));
        doneActions.Children.Add(UI.Button("Nueva ingesta", () => state.NewIngest(), prominent: state.PausedJob is null, shortcut: "ingest.new"));
        doneActions.Children.Add(UI.Button("Ver biblioteca", () => shell.Navigate("library")));
        doneActions.HorizontalAlignment = HorizontalAlignment.Center;
        var column = UI.Column(14, verb, file, animation, bar, numbers, pause, doneActions, detail);
        column.Margin = new Thickness(0, 24, 0, 0);
        var footer = state.Busy ? PageScaffold.Footer("Puedes navegar mientras tanto; sólo esta ingesta queda bloqueada.",
            UI.Button(state.Engine.IsFixture ? "Cancelar" : "Pausar", state.Cancel, destructive: true)) : null;
        return (column, footer);
    }

    public void UpdateProgress()
    {
        if (state.Progress is not { } progress || verb is null) return;
        verb.Text = progress.Verb.Length > 0 ? progress.Verb : progress.Finished ? "Listo: respaldo verificado" : "Preparando";
        file!.Text = progress.CurrentFile is null || progress.Finished ? $"{progress.FilesDone} de {progress.FilesTotal} archivos"
            : progress.From is null ? progress.CurrentFile : $"{progress.CurrentFile}  ·  {progress.From} → {progress.To}";
        bar!.Value = progress.Fraction;
        AutomationProperties.SetName(bar, $"Progreso total {progress.Fraction:P0}");
        numbers!.Text = $"{progress.Fraction:P0} · {Presentation.Bytes(progress.OverallDone)} de {Presentation.Bytes(progress.OverallTotal)} copiados y verificados · {progress.FilesDone}/{progress.FilesTotal} archivos";
        log!.Text = progress.Log.Count == 0 ? "Esperando el primer paso…" : string.Join("\n", progress.Log);
        animation!.Running = !progress.Finished && state.Busy;
        animation.Visibility = progress.Finished ? Visibility.Collapsed : Visibility.Visible;
        doneActions!.Visibility = progress.Finished ? Visibility.Visible : Visibility.Collapsed;
    }
}
