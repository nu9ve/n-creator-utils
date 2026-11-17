using System.Security.Cryptography;
using Nuun.Core.Engine;
using Xunit;

namespace Nuun.Core.Tests;

/// <summary>
/// The ingest the window runs (PLAN fase A, paso 3): `plan_document` → `apply` → cancel → `resume`, through the real
/// Rust engine and the same client the window uses. Only temporary folders: a local «card» and a local archive
/// registered with explicit consent (`allow_local`), never a real SD or disk (AGENTS.md).
/// </summary>
public class RustEngineIngestTests
{
    sealed class Fixture : IAsyncDisposable
    {
        public readonly string Root = Path.Combine(Path.GetTempPath(), "nuun-ingest-" + Guid.NewGuid().ToString("N")[..8]);
        public string State => Path.Combine(Root, "state");
        public string Source => Path.Combine(Root, "src");
        public string Archive => Path.Combine(Root, "arch");
        public RpcEngine Engine { get; private set; } = null!;

        public async Task StartAsync(int files, int megabytes)
        {
            var dcim = Path.Combine(Source, "DCIM", "100MSDCF");
            Directory.CreateDirectory(dcim);
            Directory.CreateDirectory(Archive);
            for (var i = 1; i <= files; i++)
                await File.WriteAllBytesAsync(Path.Combine(dcim, $"C000{i}.MP4"), RandomNumberGenerator.GetBytes(megabytes * 1024 * 1024));
            await EngineWrites.RegisterVolumeAsync(RustEngineFactAttribute.Binary!, State, "ARCH", "archive", Archive, null, allowLocal: true);
            var (process, connection) = EngineProcess.StartRust(RustEngineFactAttribute.Binary!, State);
            Engine = new RpcEngine(connection, new ProcessOwner(process), new EngineWorker(RustEngineFactAttribute.Binary!, State))
            {
                PollInterval = TimeSpan.FromMilliseconds(50),
            };
        }

        public PlanRequest Request(bool confirm = true) =>
            new(SourceCard.FromFolder(Source), "prueba", "ARCH", null, confirm, null, Cleanup: true);

        public int ArchivedMedia() =>
            Directory.EnumerateFiles(Path.Combine(Archive, "001_raw"), "*.MP4", SearchOption.AllDirectories).Count();

        public async ValueTask DisposeAsync()
        {
            if (Engine is not null) await Engine.DisposeAsync();
            try { Directory.Delete(Root, recursive: true); } catch (IOException) { } catch (UnauthorizedAccessException) { }
        }
    }

    sealed class Collect : IProgress<ActionProgress>
    {
        public readonly List<ActionProgress> Seen = [];
        public Action<ActionProgress>? OnReport;
        public void Report(ActionProgress value) { lock (Seen) Seen.Add(value); OnReport?.Invoke(value); }
    }

    [RustEngineFact]
    public async Task AFolderIsPlannedArchiveOnlyAppliedAndKept()
    {
        await using var f = new Fixture();
        await f.StartAsync(files: 2, megabytes: 1);

        // Archive-only is confirmed per operation; without the confirmation the engine refuses (no silent default).
        var refused = await Assert.ThrowsAsync<EngineFailure>(() => f.Engine.PlanAsync(f.Request(confirm: false)));
        Assert.Equal("confirmation_required", refused.Code);

        var plan = await f.Engine.PlanAsync(f.Request());
        Assert.NotNull(plan.Document);
        Assert.Equal(64, plan.Digest!.Length);
        Assert.Equal(2, plan.Files);
        Assert.Equal(2L * 1024 * 1024, plan.Bytes);
        Assert.Equal("prueba", plan.Project);
        // A folder is never cleaned, even if the request asked: the plan keeps the source and the button says so.
        Assert.False(plan.ReleasesSource);
        Assert.All(plan.Items, i => Assert.Equal([PlanDestination.Archive], i.Destinations));
        Assert.Contains(plan.Steps, s => s.Contains("Sin copia para editar"));

        var progress = new Collect();
        await f.Engine.ApplyAsync(plan, progress);
        var last = progress.Seen[^1];
        Assert.True(last.Finished);
        Assert.Equal("Listo: respaldo verificado; el origen se conserva", last.Verb);
        Assert.Equal(1.0, last.Fraction);
        Assert.Equal((2, 2), (last.FilesDone, last.FilesTotal));
        Assert.Contains(last.Log, l => l.StartsWith("✓ copiado y verificado"));
        Assert.Equal(2, f.ArchivedMedia());
        Assert.Equal(2, Directory.EnumerateFiles(f.Source, "*.MP4", SearchOption.AllDirectories).Count());
    }

    [RustEngineFact]
    public async Task CancellingPausesAtARecoverablePointAndResumeFinishesTheSameJob()
    {
        await using var f = new Fixture();
        await f.StartAsync(files: 3, megabytes: 48);
        var plan = await f.Engine.PlanAsync(f.Request());

        using var cancel = new CancellationTokenSource();
        var progress = new Collect();
        // Cancel once the engine reports bytes actually copied, so the pause happens mid-job.
        progress.OnReport = p => { if (!p.Finished && p.OverallDone > 0) cancel.Cancel(); };
        var paused = await Assert.ThrowsAsync<EngineFailure>(() => f.Engine.ApplyAsync(plan, progress, cancel.Token));
        Assert.Equal("cancelled", paused.Code);
        Assert.True(cancel.IsCancellationRequested);

        var job = await f.Engine.ResumeProbeAsync(plan.Id);
        Assert.Equal("paused", job);

        var resumed = new Collect();
        await f.Engine.ResumeAsync(plan.Id, resumed);
        Assert.Equal("Listo: respaldo verificado; el origen se conserva", resumed.Seen[^1].Verb);
        Assert.Equal(3, f.ArchivedMedia());
    }
}

static class RpcEngineProbe
{
    /// <summary>The job's state through a fresh one-shot read (what Biblioteca → Actividad shows).</summary>
    public static async Task<string> ResumeProbeAsync(this RpcEngine engine, string jobId)
    {
        var snapshot = await engine.SnapshotAsync();
        return snapshot.Jobs.Single(j => j.Id == jobId).State;
    }
}
