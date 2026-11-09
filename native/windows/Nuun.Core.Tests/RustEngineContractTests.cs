using Nuun.Core.Engine;
using Xunit;

namespace Nuun.Core.Tests;

/// <summary>Runs only when the Rust engine is built; otherwise xUnit reports it as skipped with the reason.</summary>
public sealed class RustEngineFactAttribute : FactAttribute
{
    public static string? Binary
    {
        get
        {
            var configured = Environment.GetEnvironmentVariable("NUUN_ENGINE_BIN");
            if (!string.IsNullOrEmpty(configured)) return File.Exists(configured) ? configured : null;
            var dir = new DirectoryInfo(AppContext.BaseDirectory);
            while (dir is not null && !Directory.Exists(Path.Combine(dir.FullName, "engine"))) dir = dir.Parent;
            var path = dir is null ? null : Path.Combine(dir.FullName, "engine", "target", "debug", "nuun-engine.exe");
            return path is not null && File.Exists(path) ? path : null;
        }
    }

    public RustEngineFactAttribute()
    {
        if (Binary is null) Skip = "Rust engine not built: cd engine && cargo build --offline";
    }
}

/// <summary>
/// Contract tests against the real Rust engine process (nove's practice: DTOs tested against the engine, not
/// mocks). The same client and DTOs the window uses for the Python engine.
/// </summary>
public class RustEngineContractTests
{
    static async Task<RpcEngine> StartAsync()
    {
        // No --state-dir: the Rust engine reads no catalog yet, so nothing is written anywhere.
        var (process, connection) = EngineProcess.StartRust(RustEngineFactAttribute.Binary!, null);
        await Task.Yield();
        return new RpcEngine(connection, new ProcessOwner(process));
    }

    [RustEngineFact]
    public async Task RecipeComesFromTheRustEngineWithThePythonShape()
    {
        await using var engine = await StartAsync();
        var cards = await engine.RecipeAsync();
        Assert.Equal(["stalker", "sony", "h6", "gopro", "insta360", "dji", "canon"], cards.Select(c => c.Family));
        Assert.Equal("sony_clips_then_format", cards.Single(c => c.Family == "sony").Cleanup);
        Assert.Null(cards[0].Cleanup);
    }

    [RustEngineFact]
    public async Task UnportedReadsSayNotPortedAndTheMaquetaAnswers()
    {
        await using var rust = await StartAsync();
        var failure = await Assert.ThrowsAsync<EngineFailure>(() => rust.SnapshotAsync());
        Assert.Equal("not_ported", failure.Code);

        await using var mixed = new MethodByMethodEngine(await StartAsync(), new FixtureEngine());
        var snapshot = await mixed.SnapshotAsync();          // falls back to fixture data
        var recipe = await mixed.RecipeAsync();              // answered by Rust
        Assert.Equal(3, snapshot.Volumes.Count);
        Assert.Equal(7, recipe.Count);
        Assert.Equal(["recipe"], mixed.AnsweredByPrimary);
        Assert.True(mixed.IsFixture);
    }

    [RustEngineFact]
    public async Task MutationsAreRefusedAsReadOnly()
    {
        var (process, connection) = EngineProcess.StartRust(RustEngineFactAttribute.Binary!, null);
        await using var owner = new ProcessOwner(process);
        await using var _ = connection;
        var failure = await Assert.ThrowsAsync<EngineFailure>(() => connection.CallAsync("apply"));
        Assert.Equal("read_only", failure.Code);
        var info = await connection.CallAsync("engine_info");
        Assert.Equal("rust", info!["engine"]!.GetValue<string>());
    }
}

/// <summary>Volume reads through the real Rust engine on Windows. Never asserts on what is mounted on this PC.</summary>
public class RustEngineVolumeTests
{
    [RustEngineFact]
    public async Task VolumesAndCardsKeepThePythonShape()
    {
        // A state folder that does not exist: an empty catalog, and nothing is created.
        var state = Path.Combine(Path.GetTempPath(), "nuun-engine-volumes-" + Guid.NewGuid().ToString("N")[..8]);
        var (process, connection) = EngineProcess.StartRust(RustEngineFactAttribute.Binary!, state);
        await using var engine = new RpcEngine(connection, new ProcessOwner(process));
        Assert.Empty(await engine.VolumesAsync());
        var cards = await engine.SourceCardsAsync();
        // No registered destination online: cards (if any are mounted) come without reading their contents.
        Assert.All(cards, card =>
        {
            Assert.False(card.HasHint);
            Assert.Equal("windows", card.IdentityMode);
            Assert.DoesNotContain("C:", card.Path, StringComparison.OrdinalIgnoreCase);
        });
        Assert.False(Directory.Exists(state));
    }
}

/// <summary>The first write of the Rust engine, end to end from C#, in a temporary state folder only.</summary>
public class RustEngineWriteTests
{
    [RustEngineFact]
    public async Task RegisteringALocalFolderNeedsConsentAndThenItIsOnline()
    {
        var temp = Path.Combine(Path.GetTempPath(), "nuun-engine-write-" + Guid.NewGuid().ToString("N")[..8]);
        var state = Path.Combine(temp, "state");
        var target = Path.Combine(temp, "destino");
        Directory.CreateDirectory(target);
        try
        {
            var refused = await Assert.ThrowsAsync<EngineFailure>(() =>
                EngineWrites.RegisterVolumeAsync(RustEngineFactAttribute.Binary!, state, "LOCAL", "work", target, null, allowLocal: false));
            Assert.Equal("external_required", refused.Code);
            Assert.Contains("carpeta interna", EngineWrites.Explain(refused));

            var record = await EngineWrites.RegisterVolumeAsync(RustEngineFactAttribute.Binary!, state, "LOCAL", "work", target, null, allowLocal: true);
            Assert.Equal("001_proyectos", record!["prefix"]!.GetValue<string>());
            Assert.Equal("windows", record["identity"]!["mode"]!.GetValue<string>());

            var duplicate = await Assert.ThrowsAsync<EngineFailure>(() =>
                EngineWrites.RegisterVolumeAsync(RustEngineFactAttribute.Binary!, state, "LOCAL", "archive", target, null, allowLocal: true));
            Assert.Equal("volume_exists", duplicate.Code);

            var (process, connection) = EngineProcess.StartRust(RustEngineFactAttribute.Binary!, state);
            await using var reader = new RpcEngine(connection, new ProcessOwner(process));
            var volume = Assert.Single(await reader.VolumesAsync());
            Assert.Equal(("LOCAL", VolumeRole.Work, true), (volume.Name, volume.Role, volume.Online));
        }
        finally
        {
            try { Directory.Delete(temp, recursive: true); } catch (IOException) { }
        }
    }
}
