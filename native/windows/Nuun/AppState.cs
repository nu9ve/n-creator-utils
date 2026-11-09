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
    public string? Error { get; private set; }

    public WindowMode Mode { get; set; } = WindowMode.Full;

    // ----- Ingest session (Entrada → Flujo → Resultado) -----
    public IngestStep Step { get; private set; } = IngestStep.Entry;
    public SourceCard? Source { get; private set; }
    public string Project { get; set; } = "proyecto-demo";
    public PlanPreview? Plan { get; private set; }
    public ActionProgress? Progress { get; private set; }
    public bool Busy { get; private set; }
    CancellationTokenSource? applying;

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
        await RefreshAsync();
    }

    public async Task RefreshAsync()
    {
        try
        {
            Snapshot = await Engine.SnapshotAsync();
            Recipe = await Engine.RecipeAsync();
            Source ??= Snapshot.Cards.FirstOrDefault();
            Changed(nameof(Snapshot));
        }
        catch (EngineFailure failure) { Report(failure.Message); }
    }

    public void Report(string message) { Error = message; Changed(nameof(Error)); }
    public void ClearError() { Error = null; Changed(nameof(Error)); }

    public void ChooseSource(SourceCard card)
    {
        if (Busy) return;
        Source = card;
        // Changing what is planned invalidates a reviewed plan (nuun-window.md).
        Plan = null;
        Changed(nameof(Source));
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
        try
        {
            Plan = await Engine.PlanAsync(Source, Project);
            Step = IngestStep.Flow;
            Changed(nameof(Step));
        }
        catch (EngineFailure failure) { Report(failure.Message); }
    }

    public async Task ApplyAsync()
    {
        if (Plan is null || Busy) return;
        Busy = true;
        applying = new CancellationTokenSource();
        Progress = new ActionProgress("Preparando", 0, 1, 0, Plan.Files, null, null, null, [], false);
        Step = IngestStep.Result;
        Changed(nameof(Step));
        try
        {
            await Engine.ApplyAsync(Plan, new Progress<ActionProgress>(p => { Progress = p; Changed(nameof(Progress)); }), applying.Token);
        }
        catch (OperationCanceledException)
        {
            Progress = Progress! with { Verb = "Cancelado: lo copiado queda verificado, nada se limpió", Finished = true };
            Changed(nameof(Progress));
        }
        catch (EngineFailure failure) { Report(failure.Message); }
        finally { Busy = false; Changed(nameof(Busy)); }
    }

    public void Cancel() => applying?.Cancel();

    public void NewIngest()
    {
        if (Busy) return;
        Plan = null; Progress = null; Step = IngestStep.Entry;
        Changed(nameof(Step));
    }

    sealed class ProcessOwner(System.Diagnostics.Process process) : IAsyncDisposable
    {
        public ValueTask DisposeAsync()
        {
            try { if (!process.HasExited) process.Kill(); } catch (InvalidOperationException) { }
            process.Dispose();
            return ValueTask.CompletedTask;
        }
    }
}
