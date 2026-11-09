using System.Text.Json.Nodes;

namespace Nuun.Core.Engine;

// Lectura tolerante del JSON del motor: un campo ausente toma un valor neutro y nunca se inventa.
// Las formas de `snapshot` (volumes, projects, jobs, source_cards + hint) y de `recipe` (cards)
// salen de media/service.py `_snapshot`, media/volumes.py `source_cards`/`card_hint` y
// media/recipes.py `summary`, leídos el 2026-11-09. El plan (`PlanPreview`) es forma de maqueta.
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
public sealed record SourceCard(string Path, string Name, string Family, string Recommendation, long UsedBytes, long CapacityBytes, string? Device)
{
    public static SourceCard Parse(JsonNode node)
    {
        var hint = node["hint"];
        var device = hint?["device"];
        return new(node.Str("path"), node.Str("name"), hint.Str("family", "generic"), hint.Str("recommendation", "archive_and_work"),
            hint.Long("used_bytes"), hint.Long("capacity_bytes"), device is null ? null : device.Str("label", device.Str("model")));
    }
}

public sealed record ProjectSummary(string Name, string Stage, int Units)
{
    public static ProjectSummary Parse(JsonNode node) => new(node.Str("name"), node.Str("stage", "draft"), (int)node.Long("units"));
}

public sealed record JobSummary(string Id, string Title, string State, bool NeedsAttention)
{
    public static JobSummary Parse(JsonNode node) => new(node.Str("id"), node.Str("title", node.Str("project")), node.Str("state"), node.Bool("attention"));
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

/// <summary>A reviewed plan as the window shows it. Maqueta shape until the Windows client reads `plan_document`.</summary>
public sealed record PlanPreview(string Id, string Project, int Files, long Bytes, IReadOnlyList<string> Steps, string? Warning, IReadOnlyList<PlanItem> Items)
{
    /// <summary>The single primary action names what it authorizes (ingest-flow-v1: one button for plan + verified cleanup).</summary>
    public bool ReleasesSource => Warning is not null;
}

/// <summary>
/// nu9ve-interaction-v1 §7 «Acciones largas»: one overall measure that never goes back
/// (`overall_done`/`overall_total`), the file in hand, and a short log emitted by the engine.
/// </summary>
public sealed record ActionProgress(string Verb, long OverallDone, long OverallTotal, int FilesDone, int FilesTotal,
    string? CurrentFile, string? From, string? To, IReadOnlyList<string> Log, bool Finished)
{
    public double Fraction => OverallTotal <= 0 ? 0 : Math.Clamp((double)OverallDone / OverallTotal, 0, 1);
}

/// <summary>The boundary every Windows view talks to. The client never recalculates plans or cleanup.</summary>
public interface IMediaEngine : IAsyncDisposable
{
    /// <summary>True for the maqueta: the UI says so and nothing touches a disk.</summary>
    bool IsFixture { get; }
    Task<Snapshot> SnapshotAsync(CancellationToken cancellation = default);
    Task<IReadOnlyList<RecipeCard>> RecipeAsync(CancellationToken cancellation = default);
    Task<PlanPreview> PlanAsync(SourceCard source, string project, CancellationToken cancellation = default);
    Task ApplyAsync(PlanPreview plan, IProgress<ActionProgress> progress, CancellationToken cancellation = default);
}
