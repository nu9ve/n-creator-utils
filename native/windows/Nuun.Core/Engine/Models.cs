using System.Text.Json.Nodes;

namespace Nuun.Core.Engine;

// Lectura tolerante del JSON del motor: un campo ausente toma un valor neutro y nunca se inventa.
// Las formas de `snapshot` (volumes, projects, jobs, source_cards + hint) y de `recipe` (cards)
// salen de media/service.py `_snapshot`, media/volumes.py `source_cards`/`card_hint` y
// media/recipes.py `summary`, leídos el 2026-11-09. El plan (`PlanPreview.FromDocument`) lee `plan_document`
// (nu9ve-ingest-plan/2) y el progreso (`ActionProgress.FromJob`) lee `job`, del motor de Rust el 2026-11-16.
static class Json
{
    public static string Str(this JsonNode? node, string key, string fallback = "") =>
        node?[key] is JsonValue v && v.TryGetValue<string>(out var s) ? s : fallback;
    public static long Long(this JsonNode? node, string key) =>
        node?[key] is JsonValue v ? (v.TryGetValue<long>(out var l) ? l : v.TryGetValue<double>(out var d) ? (long)d : 0) : 0;
    public static long? NullableLong(this JsonNode? node, string key) => node?[key] is JsonValue ? node.Long(key) : null;
    public static bool Bool(this JsonNode? node, string key) =>
        node?[key] is JsonValue v && v.TryGetValue<bool>(out var b) && b;
    public static IEnumerable<JsonNode> Items(this JsonNode? node, string key) =>
        node?[key] is JsonArray array ? array.Where(x => x is not null).Select(x => x!) : [];
}

public enum VolumeRole { Archive, Work, Other }

public sealed record Volume(string Name, VolumeRole Role, bool Online, long? Free, long? Capacity, string Prefix)
{
    public static Volume Parse(JsonNode node) => new(
        node.Str("name"),
        node.Str("role") switch { "archive" => VolumeRole.Archive, "work" => VolumeRole.Work, _ => VolumeRole.Other },
        node.Bool("online"), node.NullableLong("free"), node.NullableLong("capacity"), node.Str("prefix"));
}

/// <summary>A mounted card. `Family`/`Recommendation` come from the engine's recipe, never from the UI.</summary>
/// <remarks>
/// <see cref="HasHint"/> false: no destination was online, so the engine did not read the card's contents (product
/// rule) and <see cref="Family"/>/<see cref="Recommendation"/> are placeholders the window must not show as facts.
/// </remarks>
public sealed record SourceCard(string Path, string Name, string Family, string Recommendation, long UsedBytes, long CapacityBytes, string? Device,
    bool HasHint = true, string? IdentityMode = null, string? LastProject = null, bool Folder = false)
{
    /// <summary>A folder the person chose («Otra carpeta…»): not a mounted card, so it is never cleaned.</summary>
    public static SourceCard FromFolder(string path) =>
        new(path, System.IO.Path.GetFileName(path.TrimEnd('\\', '/')) is { Length: > 0 } name ? name : path,
            "generic", "archive_only", 0, 0, null, HasHint: false, Folder: true);

    public static SourceCard Parse(JsonNode node)
    {
        var hint = node["hint"];
        var device = hint?["device"];
        return new(node.Str("path"), node.Str("name"), hint.Str("family", "generic"), hint.Str("recommendation", "archive_and_work"),
            hint.Long("used_bytes"), hint.Long("capacity_bytes"), device is null ? null : device.Str("label", device.Str("model")),
            hint is not null, node["identity"].Str("mode", "") is { Length: > 0 } mode ? mode : null,
            hint?["last_project"] is null ? null : hint.Str("last_project"));
    }
}

public sealed record ProjectSummary(string Name, string Stage, int Units)
{
    // The fixture uses name/stage/units; the engine's `projects` (library.projects) uses project/assets and carries no stage.
    public static ProjectSummary Parse(JsonNode node) => new(node.Str("name", node.Str("project")), node.Str("stage"),
        (int)(node?["units"] is null ? node.Long("assets") : node.Long("units")));
}

public sealed record JobSummary(string Id, string Title, string State, bool NeedsAttention)
{
    /// <summary>
    /// A job the journal can continue (`resume`): paused, failed, or cut while copying, verifying or cleaning (a
    /// closed window or a crash leaves the last state written). Finished and recovered jobs are not offered.
    /// </summary>
    public bool Resumable => State is "paused" or "failed" or "planned" or "copying" or "backup_verified" or "cleaning_source";

    // Engine rows (Catalog.jobs) have no title: the short id stands in. Attention = waiting on the person and not dismissed.
    public static JobSummary Parse(JsonNode node) => new(node.Str("id"), node.Str("title", node.Str("project", node.Str("id") is { Length: > 8 } id ? id[..8] : node.Str("id"))),
        node.Str("state"), node.Bool("attention") || (node.Str("state") is "camera_action_required" or "failed" && !node.Bool("dismissed")));
}

public sealed record Snapshot(IReadOnlyList<Volume> Volumes, IReadOnlyList<SourceCard> Cards, IReadOnlyList<ProjectSummary> Projects, IReadOnlyList<JobSummary> Jobs)
{
    public static Snapshot Parse(JsonNode? node) => new(
        node.Items("volumes").Select(Volume.Parse).ToList(),
        node.Items("source_cards").Select(SourceCard.Parse).ToList(),
        node.Items("projects").Select(ProjectSummary.Parse).ToList(),
        node.Items("jobs").Select(JobSummary.Parse).ToList());
}

/// <summary>What the recipe recognizes (Flujos → «Lo que reconoce»).</summary>
public sealed record RecipeCard(string Family, string Label, string Recommendation, string? Cleanup)
{
    public static IReadOnlyList<RecipeCard> ParseSummary(JsonNode? node) =>
        node.Items("cards").Select(c => new RecipeCard(c.Str("family"), c.Str("label", c.Str("family")), c.Str("recommendation"),
            c?["cleanup"] is JsonValue ? c.Str("cleanup") : null)).ToList();
}

public enum PlanDestination { Archive, Work }

public sealed record PlanItem(string Name, string Project, long Bytes, IReadOnlyList<PlanDestination> Destinations, int Files, string Note, bool Support);

/// <summary>
/// What the window asks the engine to plan (`plan_document` parameters). Destinations are registered names. Without a
/// work destination the plan is archive-only, which the person confirms for this operation (ingest-flow-v1): the
/// copy for editing stays `pending` when a work disk is registered (delivered later from the archive) or `skipped`.
/// </summary>
public sealed record PlanRequest(SourceCard Source, string Project, string? Archive, string? Work, bool ConfirmArchiveOnly,
    string? PendingWork, bool Cleanup)
{
    public JsonObject Parameters()
    {
        var parameters = new JsonObject
        {
            ["source"] = Source.Path, ["archive"] = Archive, ["project"] = Project.Trim(),
            ["source_kind"] = "card", ["cleanup"] = Cleanup && !Source.Folder,
        };
        if (Work is { Length: > 0 }) parameters["work"] = Work;
        else
        {
            parameters["archive_only"] = true;
            parameters["confirm_archive_only"] = ConfirmArchiveOnly;
            if (PendingWork is { Length: > 0 }) { parameters["work"] = PendingWork; parameters["work_decision"] = "pending"; }
            else parameters["work_decision"] = "skipped";
        }
        return parameters;
    }
}

/// <summary>
/// A reviewed plan as the window shows it. From the engine it carries the exact `plan_document` text and its digest:
/// `apply` sends them back unchanged, so what runs is what the person reviewed. The maqueta has neither.
/// </summary>
public sealed record PlanPreview(string Id, string Project, int Files, long Bytes, IReadOnlyList<string> Steps, string? Warning, IReadOnlyList<PlanItem> Items,
    string? Document = null, string? Digest = null)
{
    /// <summary>The single primary action names what it authorizes (ingest-flow-v1: one button for plan + verified cleanup).</summary>
    public bool ReleasesSource => Warning is not null;

    /// <summary>
    /// Reads `plan_document` (schema nu9ve-ingest-plan/2): one item per unit with its files, bytes and destinations
    /// from `preview`. The client only presents: nothing here decides what is copied or cleaned.
    /// </summary>
    public static PlanPreview FromDocument(string document, string? archiveName, string? workName)
    {
        var plan = JsonNode.Parse(document) ?? throw new EngineFailure("invalid_plan", "Plan vacío");
        var digest = plan.Str("digest");
        if (digest.Length == 0) throw new EngineFailure("invalid_plan", "El plan recibido no tiene digest");
        var rows = plan.Items("preview").ToDictionary(r => r.Str("source"), r => r);
        var items = new List<PlanItem>();
        foreach (var unit in plan.Items("units"))
        {
            var paths = unit.Items("paths").Select(p => p.GetValue<string>()).ToList();
            var unitRows = paths.Where(rows.ContainsKey).Select(p => rows[p]).ToList();
            if (unitRows.Count == 0) continue;
            var destinations = new List<PlanDestination> { PlanDestination.Archive };
            if (unitRows.Any(r => r["work_path"] is JsonValue)) destinations.Add(PlanDestination.Work);
            var first = paths[0][(paths[0].LastIndexOf('/') + 1)..];
            var name = paths.Count == 1 ? first : $"{first} y {paths.Count - 1} más";
            var support = unit.Bool("support");
            var actions = unitRows.Select(r => r.Str("source_action")).ToHashSet();
            var note = actions.Contains("release_verified") || actions.Contains("release_clipper") ? "Liberar tras verificar"
                : actions.Contains("camera_format") ? "Conservar índices · formato en cámara"
                : "Se conserva en el origen";
            items.Add(new(name, unit.Str("project", plan.Str("project")), unitRows.Sum(r => r.Long("bytes")), destinations, unitRows.Count, note, support));
        }
        var steps = new List<string> { $"Respaldo verificado en {archiveName ?? "el archivo"}" };
        steps.Add(plan.Str("work_delivery_status") switch
        {
            "ready" => $"Copia para editar en {workName ?? "el disco de trabajo"}, verificada antes de limpiar",
            "pending" => "Copia para editar pendiente: se entrega después desde el archivo",
            _ => "Sin copia para editar en esta operación",
        });
        var cleanup = plan.Bool("cleanup");
        string? warning = !cleanup ? null
            : plan.Str("cleanup_profile") == "camera_action_required"
                ? "Después de verificar se retiran los clips; termina con formato en la cámara."
                : "Después de verificar todos los destinos se vacía la tarjeta.";
        var project = items.Select(i => i.Project).FirstOrDefault(p => p.Length > 0) ?? "";
        return new(plan.Str("id"), project, items.Sum(i => i.Files), items.Sum(i => i.Bytes), steps, warning, items, document, digest);
    }
}

/// <summary>
/// nu9ve-interaction-v1 §7 «Acciones largas»: one overall measure that never goes back
/// (`overall_done`/`overall_total`), the file in hand, and a short log emitted by the engine.
/// </summary>
public sealed record ActionProgress(string Verb, long OverallDone, long OverallTotal, int FilesDone, int FilesTotal,
    string? CurrentFile, string? From, string? To, IReadOnlyList<string> Log, bool Finished)
{
    public double Fraction => OverallTotal <= 0 ? 0 : Math.Clamp((double)OverallDone / OverallTotal, 0, 1);

    /// <summary>
    /// A `job` row as the Result view shows it. The measure is the engine's own `overall_done`/`overall_total`
    /// (read + verify on every destination + reread before cleanup); the client never invents a percentage.
    /// </summary>
    public static ActionProgress FromJob(JsonNode? job, bool finished)
    {
        var p = job?["progress"];
        var state = job.Str("state");
        var verb = finished ? Final(state) : p.Str("phase") switch
        {
            "copying" => "Copiando y verificando",
            "verifying" => "Verificando destinos",
            "cleaning" => "Liberando el origen tras verificar",
            "checking_card" => "Comprobando la tarjeta",
            "converting" => "Convirtiendo a MP4",
            _ => "Preparando",
        };
        var current = p?["current"];
        var log = p.Items("log").Select(entry => entry.Str("kind") switch
        {
            "copied" => $"✓ copiado y verificado {entry.Str("name")} → {entry.Str("target")}",
            "cleaned" => $"− liberado {entry.Str("name")} tras verificar",
            var kind => $"{kind} {entry.Str("name")} {entry.Str("target")}".Trim(),
        }).ToList();
        var total = p.Long("overall_total") is > 0 and var t ? t : p.Long("global_total");
        var done = p.Long("overall_total") > 0 ? p.Long("overall_done") : p.Long("global_done");
        return new(verb, done, total, (int)p.Long("files_done"), (int)p.Long("files_total"),
            current.Str("name") is { Length: > 0 } name ? name : null,
            current.Str("from") is { Length: > 0 } from ? from : null,
            current.Str("to") is { Length: > 0 } to ? to : null, log, finished);
    }

    static string Final(string state) => state switch
    {
        "completed" => "Listo: respaldo verificado y origen liberado",
        "source_retained" => "Listo: respaldo verificado; el origen se conserva",
        "camera_action_required" => "Respaldo verificado: falta formatear en la cámara",
        "paused" => "En pausa: lo copiado queda verificado, nada se limpió",
        var other => $"Terminó en «{other}»",
    };
}

/// <summary>The boundary every Windows view talks to. The client never recalculates plans or cleanup.</summary>
public interface IMediaEngine : IAsyncDisposable
{
    /// <summary>True for the maqueta: the UI says so and nothing touches a disk.</summary>
    bool IsFixture { get; }
    Task<Snapshot> SnapshotAsync(CancellationToken cancellation = default);
    Task<IReadOnlyList<RecipeCard>> RecipeAsync(CancellationToken cancellation = default);
    /// <summary>Registered volumes with `online` (the `volumes` RPC).</summary>
    Task<IReadOnlyList<Volume>> VolumesAsync(CancellationToken cancellation = default);
    /// <summary>Mounted cards (the `source_cards` RPC); `hint` only when a destination is online.</summary>
    Task<IReadOnlyList<SourceCard>> SourceCardsAsync(CancellationToken cancellation = default);
    Task<PlanPreview> PlanAsync(PlanRequest request, CancellationToken cancellation = default);
    /// <summary>
    /// Runs the reviewed plan. Cancelling stops at a recoverable point: the job is left `paused` and the call throws
    /// <see cref="EngineFailure"/> `cancelled`; <see cref="ResumeAsync"/> continues the same job.
    /// </summary>
    Task ApplyAsync(PlanPreview plan, IProgress<ActionProgress> progress, CancellationToken cancellation = default);
    /// <summary>Continues an interrupted or paused job from its journal (`resume`).</summary>
    Task ResumeAsync(string jobId, IProgress<ActionProgress> progress, CancellationToken cancellation = default);
}

/// <summary>
/// What `discover` found on a source: units (a clip with its sidecars, a photo pair, a Stalker session, or the
/// camera's support files), total bytes, the devices read from the stills' EXIF (label, else FUENTE tag), how many
/// units wait for the user to confirm their device, and what the engine cannot look up yet (`not_ported`, e.g.
/// `mission`).
/// </summary>
public sealed record Discovery(int Units, long Bytes, IReadOnlyDictionary<string, int> Categories, string Camera,
    IReadOnlyList<string> Skipped, IReadOnlyList<string> NotPorted, IReadOnlyList<string> Devices, int ToConfirm)
{
    public static Discovery Parse(JsonNode? node)
    {
        var units = node.Items("units").ToList();
        var categories = units.GroupBy(u => u.Str("category")).ToDictionary(g => g.Key, g => g.Count());
        var camera = units.Select(u => u.Str("camera")).FirstOrDefault(c => c.Length > 0) ?? "";
        var devices = units.Select(u => u["device"]).Where(d => d is not null)
            .Select(d => d.Str("label") is { Length: > 0 } label ? label : d.Str("source"))
            .Where(name => name.Length > 0).Distinct().ToList();
        var toConfirm = units.Count(u => u["device"]?["requires_confirmation"]?.GetValue<bool>() == true);
        return new(units.Count, node.Long("bytes"), categories, camera,
            node.Items("skipped").Select(n => n.GetValue<string>()).ToList(),
            node.Items("not_ported").Select(n => n.GetValue<string>()).ToList(), devices, toConfirm);
    }
}
