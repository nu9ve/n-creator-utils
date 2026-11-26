using System.ComponentModel;
using Microsoft.UI.Dispatching;
using Nu9ve.Shell;
using Nuun.Core.Shell;
using Nuun.Core.Engine;

namespace Nuun;

public enum IngestStep { Entry, Flow, Result }

/// <summary>
/// The window's state, shared by every mode and page (docs/features/nuun-window.md: one model, three
/// presentations). It holds what the engine said; it does not compute plans or cleanup.
/// </summary>
public sealed class AppState(DispatcherQueue dispatcher) : INotifyPropertyChanged
{
    public event PropertyChangedEventHandler? PropertyChanged;
    void Changed(string name) => dispatcher.TryEnqueue(() => PropertyChanged?.Invoke(this, new(name)));

    public IMediaEngine Engine { get; private set; } = new FixtureEngine();
    public Snapshot Snapshot { get; private set; } = new([], [], [], []);
    public IReadOnlyList<RecipeCard> Recipe { get; private set; } = [];
    /// <summary>What the Rust engine itself read from this PC (never fixture data); null when not ported here.</summary>
    public IReadOnlyList<Volume>? LiveVolumes { get; private set; }
    public IReadOnlyList<SourceCard>? LiveCards { get; private set; }
    public string? RustBinary { get; private set; }
    public string? StateDirectory { get; private set; }

    /// <summary>Registers a destination through the Rust engine (configuration only), then reads again.</summary>
    public async Task<string?> RegisterDestinationAsync(string name, string role, string root, string? prefix, bool allowLocal)
    {
        if (RustBinary is null) return "El motor de Rust no está disponible.";
        try
        {
            await EngineWrites.RegisterVolumeAsync(RustBinary, StateDirectory, name, role, root, prefix, allowLocal);
            await RefreshAsync();
            return null;
        }
        catch (EngineFailure failure) { return EngineWrites.Explain(failure); }
    }
    public string? Error { get; private set; }

    public WindowMode Mode { get; set; } = WindowMode.Full;

    // ----- Ingest session (Entrada → Flujo → Resultado) -----
    public IngestStep Step { get; private set; } = IngestStep.Entry;
    public SourceCard? Source { get; private set; }
    public string Project { get; set; } = "proyecto-demo";
    public PlanPreview? Plan { get; private set; }
    public ActionProgress? Progress { get; private set; }
    public bool Busy { get; private set; }
    /// <summary>The job the last run left paused or interrupted: Resultado offers «Reanudar».</summary>
    public string? PausedJob { get; private set; }
    CancellationTokenSource? applying;

    // ----- Destinations for the plan (real engine only; the maqueta shows its demo disks) -----
    /// <summary>Registered archive destination chosen for the plan (its name, as the engine's `archive` parameter).</summary>
    public string? Archive { get; set; }
    /// <summary>Work destination for the copy for editing; null means archive-only for this operation.</summary>
    public string? Work { get; set; }
    /// <summary>Archive-only is confirmed per operation, never a default (ingest-flow-v1).</summary>
    public bool ConfirmArchiveOnly { get; set; }
    /// <summary>Release the card after every required destination is verified. Never for a chosen folder.</summary>
    public bool Cleanup { get; set; } = true;

    /// <summary>Registered destinations online now, by role, as the engine read them.</summary>
    public IReadOnlyList<Volume> OnlineDestinations(VolumeRole role) =>
        (LiveVolumes ?? []).Where(v => v.Role == role && v.Online).ToList();

    void ChooseDefaultDestinations()
    {
        var archives = OnlineDestinations(VolumeRole.Archive);
        if (Archive is null || archives.All(v => v.Name != Archive)) Archive = archives.FirstOrDefault()?.Name;
        var works = OnlineDestinations(VolumeRole.Work);
        if (Work is not null && works.All(v => v.Name != Work)) Work = null;
    }

    /// <summary>
    /// Without a work copy now, a registered work disk keeps it `pending` (delivered later from the archive); with none
    /// registered it is `skipped`. The plan shows which.
    /// </summary>
    string? PendingWork => Work is null ? (LiveVolumes ?? []).FirstOrDefault(v => v.Role == VolumeRole.Work)?.Name : null;

    public async Task StartAsync()
    {
        // The real engine only when configured; otherwise the maqueta, which the window labels as such.
        if (EngineConfiguration.FromEnvironment() is { } configuration)
        {
            try
            {
                var (process, connection) = EngineProcess.Start(configuration);
                Engine = new RpcEngine(connection, new ProcessOwner(process));
            }
            catch (Exception error) when (error is EngineFailure or System.ComponentModel.Win32Exception)
            {
                Report($"Motor no disponible, se usa la maqueta: {error.Message}");
            }
        }
        // The Rust engine answers what it has ported; the maqueta answers the rest (rust-engine.md §7).
        else if (!Program.RequestedMaqueta && EngineProcess.FindRustEngine() is { } rust)
        {
            try
            {
                RustBinary = rust;
                // QA keeps the catalog under its own root; otherwise the engine's default (%LOCALAPPDATA%/n-creator-utils/media).
                StateDirectory = Environment.GetEnvironmentVariable("NUUN_QA_ROOT") is { Length: > 0 } qa ? Path.Combine(qa, "state") : null;
                var (process, connection) = EngineProcess.StartRust(rust, StateDirectory);
                // Writes (plan_document, apply, resume) each run in their own `rpc` worker of the same binary and catalog.
                Engine = new MethodByMethodEngine(new RpcEngine(connection, new ProcessOwner(process), new EngineWorker(rust, StateDirectory)),
                    new FixtureEngine());
            }
            catch (Exception error) when (error is EngineFailure or System.ComponentModel.Win32Exception)
            {
                Report($"Motor de Rust no disponible, se usa la maqueta: {error.Message}");
            }
        }
        await RefreshAsync();
        // «proyecto-demo» belongs to the maqueta. With real data the batch starts without a project (the engine's
        // general project) unless the card remembers its last one.
        if (!Engine.IsFixture && Project == "proyecto-demo") { Project = Source?.LastProject ?? ""; Changed(nameof(Source)); }
    }

    public async Task RefreshAsync()
    {
        try
        {
            Snapshot = await Engine.SnapshotAsync();
            Recipe = await Engine.RecipeAsync();
            if (Engine is MethodByMethodEngine mixed)
            {
                LiveVolumes = await mixed.PrimaryOnlyAsync(e => e.VolumesAsync());
                LiveCards = await mixed.PrimaryOnlyAsync(e => e.SourceCardsAsync());
                ChooseDefaultDestinations();
            }
            Source ??= Snapshot.Cards.FirstOrDefault();
            Changed(nameof(Snapshot));
        }
        catch (EngineFailure failure) { Report(failure.Message); }
    }

    public void Report(string message) { Error = message; Changed(nameof(Error)); }
    public void ClearError() { Error = null; Changed(nameof(Error)); }

    /// <summary>«Otra carpeta…»: a folder is backed up and never cleaned (only a mounted card is released).</summary>
    public void UseFolder(string path)
    {
        if (Busy || string.IsNullOrWhiteSpace(path)) return;
        var full = path.Trim();
        if (!Directory.Exists(full)) { Report("Esa carpeta no existe o no está disponible."); return; }
        ChooseSource(SourceCard.FromFolder(full));
    }

    /// <summary>Choosing destinations or options invalidates a reviewed plan, like choosing another source.</summary>
    public void ChangePlanOptions(Action change)
    {
        if (Busy) return;
        change();
        Plan = null;
        Changed(nameof(Source));
    }

    public void ChooseSource(SourceCard card)
    {
        if (Busy) return;
        Source = card;
        ConfirmArchiveOnly = false;
        // Changing what is planned invalidates a reviewed plan (nuun-window.md).
        Plan = null;
        Discovery = null;
        Changed(nameof(Source));
    }

    /// <summary>What the Rust engine read on the chosen real card; null for the maqueta or before reading.</summary>
    public Discovery? Discovery { get; private set; }

    /// <summary>
    /// Reads the chosen card's contents with `discover`. Only on request (choosing a source never deep-scans, as on
    /// Mac) and only for real data: the maqueta keeps its fixture summary.
    /// </summary>
    public async Task DiscoverAsync()
    {
        if (Source is null || RustBinary is null || Engine.IsFixture || Busy) return;
        try
        {
            Discovery = await EngineWrites.DiscoverAsync(RustBinary, StateDirectory, Source.Path);
            Changed(nameof(Source));
        }
        catch (EngineFailure failure) { Report(EngineWrites.Explain(failure)); }
    }

    public void Go(IngestStep step)
    {
        if (Busy && step != IngestStep.Result) return;
        if (step == IngestStep.Flow && Plan is null) { _ = ReviewAsync(); return; }
        if (step == IngestStep.Result && Progress is null) return;
        Step = step;
        Changed(nameof(Step));
    }

    public async Task ReviewAsync()
    {
        if (Source is null || Busy) return;
        if (!Engine.IsFixture && Archive is null)
        {
            Report("Registra y conecta un destino de archivo (Biblioteca → Discos) antes de planear.");
            return;
        }
        var request = Engine.IsFixture
            ? new PlanRequest(Source, Project, "MEDIA-DEMO", "EDICION-DEMO", false, null, true)
            : new PlanRequest(Source, Project, Archive, Work, ConfirmArchiveOnly, PendingWork, Cleanup && !Source.Folder);
        try
        {
            Plan = await Engine.PlanAsync(request);
            ClearError();
            Step = IngestStep.Flow;
            Changed(nameof(Step));
        }
        catch (EngineFailure failure) { Report(EngineWrites.Explain(failure)); }
    }

    public Task ApplyAsync() => Plan is { } plan ? RunAsync(plan.Id, plan.Files, (p, c) => Engine.ApplyAsync(plan, p, c)) : Task.CompletedTask;

    /// <summary>Continues a paused or interrupted job from its journal; the copies already verified are not read again.</summary>
    public Task ResumeAsync(string jobId) => RunAsync(jobId, 0, (p, c) => Engine.ResumeAsync(jobId, p, c));

    async Task RunAsync(string jobId, int files, Func<IProgress<ActionProgress>, CancellationToken, Task> run)
    {
        if (Busy) return;
        Busy = true;
        PausedJob = null;
        ClearError();
        var cancellation = new CancellationTokenSource();
        applying = cancellation;
        Progress = new ActionProgress("Preparando", 0, 1, 0, files, null, null, null, [], false);
        Step = IngestStep.Result;
        Changed(nameof(Step));
        var finished = false;
        ActionProgress Stopped(string verb) =>
            (Progress ?? new ActionProgress("", 0, 1, 0, 0, null, null, null, [], false)) with { Verb = verb, Finished = true };
        try
        {
            // Progress<T> posts to the UI thread: a late poll never replaces the final report.
            await run(new Progress<ActionProgress>(p =>
            {
                if (finished && !p.Finished) return;
                // While pausing, the copy in hand still advances: keep saying what was asked.
                Progress = !p.Finished && cancellation.IsCancellationRequested ? p with { Verb = "Pausando en un punto recuperable…" } : p;
                Changed(nameof(Progress));
            }), cancellation.Token);
            finished = true;
            if (!Engine.IsFixture) Plan = null;
        }
        catch (EngineFailure failure) when (failure.Code == "cancelled")
        {
            finished = true;
            PausedJob = jobId;
            Progress = Stopped("En pausa: lo copiado queda verificado, nada se limpió");
            Changed(nameof(Progress));
        }
        catch (OperationCanceledException)
        {
            // Only the maqueta cancels by token: it never wrote anything.
            finished = true;
            Progress = Stopped("Cancelado (maqueta): nada se copió ni se limpió");
            Changed(nameof(Progress));
        }
        catch (EngineFailure failure)
        {
            finished = true;
            if (!Engine.IsFixture) PausedJob = jobId;
            Progress = Stopped("Se detuvo: nada se limpió sin verificar");
            Changed(nameof(Progress));
            Report(EngineWrites.Explain(failure));
        }
        finally
        {
            Busy = false;
            applying = null;
            cancellation.Dispose();
            Changed(nameof(Busy));
        }
        if (!Engine.IsFixture) await RefreshAsync();
    }

    /// <summary>Asks the engine to pause at the next recoverable point; the copies already verified stay.</summary>
    public void Cancel()
    {
        if (applying is not { IsCancellationRequested: false } running) return;
        running.Cancel();
        if (Progress is { } p) { Progress = p with { Verb = "Pausando en un punto recuperable…" }; Changed(nameof(Progress)); }
    }

    public void NewIngest()
    {
        if (Busy) return;
        Plan = null; Progress = null; PausedJob = null; Step = IngestStep.Entry;
        // Archive-only is confirmed per operation (ingest-flow-v1): a new ingest asks again.
        ConfirmArchiveOnly = false;
        Changed(nameof(Step));
    }
}
