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
