using System.Reflection;
using System.Text.Json.Nodes;

namespace Nuun.Core.Engine;

/// <summary>
/// Maqueta: datos ficticios con la forma del motor real, para ver temas y pantallas en Windows sin
/// Python ni discos. No lee ni escribe ningún volumen; «aplicar» sólo emite progreso simulado,
/// y la UI lo rotula como maqueta. Nunca usa SDs, ESSD ni MEDIA reales (AGENTS.md).
/// </summary>
public sealed class FixtureEngine(TimeSpan? step = null) : IMediaEngine
{
    readonly TimeSpan step = step ?? TimeSpan.FromMilliseconds(140);
    public bool IsFixture => true;

    static JsonNode? Load(string name)
    {
        var assembly = typeof(FixtureEngine).Assembly;
        var resource = assembly.GetManifestResourceNames().Single(n => n.EndsWith($"Fixtures.{name}", StringComparison.Ordinal));
        using var stream = assembly.GetManifestResourceStream(resource)!;
        return JsonNode.Parse(stream);
    }

    public Task<Snapshot> SnapshotAsync(CancellationToken cancellation = default) =>
        Task.FromResult(Snapshot.Parse(Load("snapshot.json")));

    public Task<IReadOnlyList<RecipeCard>> RecipeAsync(CancellationToken cancellation = default) =>
        Task.FromResult(RecipeCard.ParseSummary(Load("recipe.json")));

    public async Task<IReadOnlyList<Volume>> VolumesAsync(CancellationToken cancellation = default) =>
        (await SnapshotAsync(cancellation).ConfigureAwait(false)).Volumes;

    public async Task<IReadOnlyList<SourceCard>> SourceCardsAsync(CancellationToken cancellation = default) =>
        (await SnapshotAsync(cancellation).ConfigureAwait(false)).Cards;

    public Task<PlanPreview> PlanAsync(SourceCard source, string project, CancellationToken cancellation = default)
    {
        var name = string.IsNullOrWhiteSpace(project) ? "proyecto-demo" : project.Trim();
        // Same plan the Mac README screenshot shows (fixture data): one clip and the camera's support files.
        var sony = source.Family == "sony";
        PlanItem[] items = sony
            ? [new("C0001.MP4", name, 4_200_000, [PlanDestination.Archive, PlanDestination.Work], 1, "Liberar tras verificar", false),
               new("Datos de cámara", name, 8_000, [PlanDestination.Archive], 1, "Conservar índices · formato en cámara", true)]
            : [new("DJI_0001.JPG", name, 9_400_000, [PlanDestination.Archive, PlanDestination.Work], 1, "Liberar tras verificar", false),
               new("DJI_0002.JPG", name, 9_100_000, [PlanDestination.Archive, PlanDestination.Work], 1, "Liberar tras verificar", false),
               new("Registro de vuelo", name, 64_000, [PlanDestination.Archive], 1, "Sólo respaldo", true)];
        string[] steps = ["Respaldo en MEDIA", "Copias para editar en SSD verificadas antes de limpiar"];
        var warning = sony ? "Después de verificar se borran los clips; termina con formato en la cámara." : "Después de verificar se vacía la tarjeta.";
        return Task.FromResult(new PlanPreview($"fixture-{Math.Abs(source.Path.GetHashCode()) % 100000:00000}", name,
            items.Sum(i => i.Files), items.Sum(i => i.Bytes), steps, warning, items));
    }

    public async Task ApplyAsync(PlanPreview plan, IProgress<ActionProgress> progress, CancellationToken cancellation = default)
    {
        // Read + verify on each destination + reread before cleanup, as one measure (ActionMeter).
        // Bytes copied and verified on every destination: the plan size times its destinations.
        var total = plan.Items.Sum(i => i.Bytes * i.Destinations.Count);
        long done = 0;
        var files = 0;
        var log = new List<string>();
        foreach (var item in plan.Items)
        {
            foreach (var destination in item.Destinations)
            {
                var target = destination == PlanDestination.Archive ? "MEDIA" : "ESSD";
                const int slices = 8;
                for (var s = 0; s < slices; s++)
                {
                    cancellation.ThrowIfCancellationRequested();
                    done += item.Bytes / slices;
                    progress.Report(new("Copiando y verificando", Math.Min(done, total), total, files, plan.Files,
                        item.Name, "SD", target, [.. log], false));
                    await Task.Delay(step, cancellation).ConfigureAwait(false);
                }
                log.Add($"✓ copiado y verificado {item.Name} → {target}");
            }
            files++;
        }
        log.Add(plan.ReleasesSource ? "− liberado de la tarjeta tras verificar (maqueta: no se borró nada)" : "= la tarjeta se conserva");
        progress.Report(new("Listo", total, total, files, plan.Files, null, null, null, [.. log], true));
    }

    public ValueTask DisposeAsync() => ValueTask.CompletedTask;
}

/// <summary>The real engine over `media rpc-serve`. Only `snapshot` and `recipe` are wired on Windows today.</summary>
public sealed class RpcEngine(RpcConnection connection, IAsyncDisposable? owner = null) : IMediaEngine
{
    public bool IsFixture => false;

    public async Task<Snapshot> SnapshotAsync(CancellationToken cancellation = default) =>
        Snapshot.Parse(await connection.CallAsync("snapshot", null, cancellation).ConfigureAwait(false));

    public async Task<IReadOnlyList<RecipeCard>> RecipeAsync(CancellationToken cancellation = default) =>
        RecipeCard.ParseSummary(await connection.CallAsync("recipe", null, cancellation).ConfigureAwait(false));

    public async Task<IReadOnlyList<Volume>> VolumesAsync(CancellationToken cancellation = default) =>
        (await connection.CallAsync("volumes", null, cancellation).ConfigureAwait(false) as JsonArray ?? []).Where(n => n is not null).Select(n => Volume.Parse(n!)).ToList();

    public async Task<IReadOnlyList<SourceCard>> SourceCardsAsync(CancellationToken cancellation = default) =>
        (await connection.CallAsync("source_cards", null, cancellation).ConfigureAwait(false) as JsonArray ?? []).Where(n => n is not null).Select(n => SourceCard.Parse(n!)).ToList();

    // Planning and applying go through `plan_document`/`apply` with the reviewed digest, as on Mac. Not wired
    // on Windows: the volume layer (media/volumes.py) is macOS-specific (diskutil/DiskArbitration).
    public Task<PlanPreview> PlanAsync(SourceCard source, string project, CancellationToken cancellation = default) =>
        throw new EngineFailure("not_on_windows", "Planear desde Windows necesita el adaptador de volúmenes de Windows");

    public Task ApplyAsync(PlanPreview plan, IProgress<ActionProgress> progress, CancellationToken cancellation = default) =>
        throw new EngineFailure("not_on_windows", "Aplicar desde Windows necesita el adaptador de volúmenes de Windows");

    public async ValueTask DisposeAsync()
    {
        await connection.DisposeAsync().ConfigureAwait(false);
        if (owner is not null) await owner.DisposeAsync().ConfigureAwait(false);
    }
}

/// <summary>
/// Method by method (rust-engine.md §7): ask <paramref name="primary"/> and, when it answers `not_ported`, ask
/// <paramref name="fallback"/>. This is how the Windows window uses the Rust engine while it grows, and how the
/// Mac app will switch from Python one method at a time. The window still says «Maqueta» while any read falls
/// back to fixture data.
/// </summary>
public sealed class MethodByMethodEngine(IMediaEngine primary, IMediaEngine fallback) : IMediaEngine
{
    readonly HashSet<string> answeredByPrimary = [];
    /// <summary>Still a maqueta until the primary engine answers `snapshot` with this PC's real catalog.</summary>
    public bool IsFixture => fallback.IsFixture && !RealSnapshot;
    /// <summary>The views show real data: plans and applies must never fall back to the simulated ones.</summary>
    public bool RealSnapshot { get { lock (answeredByPrimary) return answeredByPrimary.Contains("snapshot"); } }
    /// <summary>Methods the primary engine answered so far (shown in Ajustes → Motor).</summary>
    public IReadOnlyCollection<string> AnsweredByPrimary => answeredByPrimary;

    async Task<T> Try<T>(string method, Func<IMediaEngine, Task<T>> call)
    {
        try
        {
            var value = await call(primary).ConfigureAwait(false);
            lock (answeredByPrimary) answeredByPrimary.Add(method);
            return value;
        }
        catch (EngineFailure failure) when (failure.Code is "not_ported" or "not_on_windows")
        {
            return await call(fallback).ConfigureAwait(false);
        }
    }

    public Task<Snapshot> SnapshotAsync(CancellationToken cancellation = default) => Try("snapshot", e => e.SnapshotAsync(cancellation));
    public Task<IReadOnlyList<RecipeCard>> RecipeAsync(CancellationToken cancellation = default) => Try("recipe", e => e.RecipeAsync(cancellation));
    public Task<IReadOnlyList<Volume>> VolumesAsync(CancellationToken cancellation = default) => Try("volumes", e => e.VolumesAsync(cancellation));
    public Task<IReadOnlyList<SourceCard>> SourceCardsAsync(CancellationToken cancellation = default) => Try("source_cards", e => e.SourceCardsAsync(cancellation));

    /// <summary>Only what the primary engine itself answers (never the fallback): real data, or null when not ported.</summary>
    public async Task<T?> PrimaryOnlyAsync<T>(Func<IMediaEngine, Task<T>> call) where T : class
    {
        try { return await call(primary).ConfigureAwait(false); }
        catch (EngineFailure failure) when (failure.Code is "not_ported" or "not_on_windows") { return null; }
    }
    // A simulated plan of a real card would look like a backup that never happened: with a real snapshot, plan and
    // apply go to the primary engine only and its `not_ported` reaches the person.
    public Task<PlanPreview> PlanAsync(SourceCard source, string project, CancellationToken cancellation = default) =>
        RealSnapshot ? primary.PlanAsync(source, project, cancellation) : Try("plan", e => e.PlanAsync(source, project, cancellation));
    public Task ApplyAsync(PlanPreview plan, IProgress<ActionProgress> progress, CancellationToken cancellation = default) =>
        RealSnapshot ? primary.ApplyAsync(plan, progress, cancellation)
            : Try("apply", async e => { await e.ApplyAsync(plan, progress, cancellation).ConfigureAwait(false); return true; });

    public async ValueTask DisposeAsync()
    {
        await primary.DisposeAsync().ConfigureAwait(false);
        await fallback.DisposeAsync().ConfigureAwait(false);
    }
}
