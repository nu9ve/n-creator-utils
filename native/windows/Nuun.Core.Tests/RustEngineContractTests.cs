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
        // A state folder that does not exist: an empty catalog, never this PC's own, and reads create nothing.
        var state = Path.Combine(Path.GetTempPath(), "nuun-engine-test-" + Guid.NewGuid().ToString("N")[..8]);
        var (process, connection) = EngineProcess.StartRust(RustEngineFactAttribute.Binary!, state);
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
    public async Task ARealSnapshotEndsTheMaquetaAndNeverSimulatesAPlan()
    {
        await using var mixed = new MethodByMethodEngine(await StartAsync(), new FixtureEngine());
        var snapshot = await mixed.SnapshotAsync();          // answered by Rust: this PC's (empty) catalog
        var recipe = await mixed.RecipeAsync();
        Assert.Empty(snapshot.Volumes);
        Assert.Empty(snapshot.Jobs);
        Assert.Equal(7, recipe.Count);
        Assert.Equal(["recipe", "snapshot"], mixed.AnsweredByPrimary.Order());
        Assert.False(mixed.IsFixture);
        Assert.True(mixed.RealSnapshot);
        // With real data on screen a simulated plan would look like a backup that never happened.
        var card = new SourceCard("E:\\", "CARD", "sony", "sony_format_in_camera", 0, 0, null);
        var failure = await Assert.ThrowsAsync<EngineFailure>(() => mixed.PlanAsync(card, "proyecto"));
        Assert.Equal("not_on_windows", failure.Code);
    }

    [RustEngineFact]
    public async Task SnapshotNamesWhatIsNotPortedYet()
    {
        var (process, connection) = EngineProcess.StartRust(RustEngineFactAttribute.Binary!, Path.Combine(Path.GetTempPath(), "nuun-snapshot-" + Guid.NewGuid().ToString("N")[..8]));
        await using var owner = new ProcessOwner(process);
        await using var _ = connection;
        var data = await connection.CallAsync("snapshot", new System.Text.Json.Nodes.JsonObject { ["cards"] = false });
        Assert.StartsWith("not_ported", data!["errors"]!["capabilities"]!.GetValue<string>());
        Assert.Equal("hermosillo", data["defaults"]!["city"]!.GetValue<string>());
        Assert.Null(data["source_cards"]);
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

/// <summary>`discover` through the real Rust engine, on a temporary card tree only.</summary>
public class RustEngineDiscoverTests
{
    [RustEngineFact]
    public async Task DiscoverGroupsAClipWithItsSidecarAndReadsTheStillsDevice()
    {
        var card = Path.Combine(Path.GetTempPath(), "nuun-discover-" + Guid.NewGuid().ToString("N")[..8]);
        try
        {
            Directory.CreateDirectory(Path.Combine(card, "PRIVATE", "M4ROOT", "CLIP"));
            Directory.CreateDirectory(Path.Combine(card, "DCIM", "100MSDCF"));
            File.WriteAllText(Path.Combine(card, "PRIVATE", "M4ROOT", "CLIP", "C0001.MP4"), "video");
            File.WriteAllText(Path.Combine(card, "PRIVATE", "M4ROOT", "CLIP", "C0001M01.XML"),
                "<NonRealTimeMeta><Device manufacturer=\"Sony\" modelName=\"ILCE-6400\"/></NonRealTimeMeta>");
            File.WriteAllBytes(Path.Combine(card, "DCIM", "100MSDCF", "DSC0001.JPG"), ExifJpeg("DJI ", "FC3170", "2026:05:14 10:20:30"));
            File.WriteAllText(Path.Combine(card, "DCIM", "100MSDCF", "DSC0002.JPG"), "not a photo");
            var found = await EngineWrites.DiscoverAsync(RustEngineFactAttribute.Binary!, Path.Combine(card + "-state"), card);
            Assert.Equal(3, found.Units);
            Assert.Equal(1, found.Categories["video"]);
            Assert.Equal(2, found.Categories["image"]);
            Assert.Equal("ILCE-6400", found.Camera);
            Assert.Equal(["DJI Mavic Air 2"], found.Devices);
            Assert.Equal(0, found.ToConfirm);
            Assert.Empty(found.NotPorted); // `mission` only with an archive
        }
        finally
        {
            try { Directory.Delete(card, recursive: true); } catch (IOException) { }
            try { Directory.Delete(card + "-state", recursive: true); } catch (IOException) { }
        }
    }

    /// <summary>A minimal JPEG whose EXIF holds Make, Model and DateTimeOriginal (little-endian TIFF).</summary>
    static byte[] ExifJpeg(string make, string model, string when)
    {
        var tiff = new List<byte>();
        void U16(int v) { tiff.Add((byte)v); tiff.Add((byte)(v >> 8)); }
        void U32(int v) { U16(v & 0xFFFF); U16(v >> 16); }
        byte[] Ascii(string s) => [.. System.Text.Encoding.ASCII.GetBytes(s), 0];
        var (mk, md, wh) = (Ascii(make), Ascii(model), Ascii(when));
        if (mk.Length <= 4 || md.Length <= 4) throw new ArgumentException("make and model need 4+ characters here");
        const int ifd0 = 8, exif = ifd0 + 2 + 12 * 3 + 4, data = exif + 2 + 12 + 4;
        tiff.AddRange("II"u8.ToArray()); U16(42); U32(ifd0);
        U16(3);
        U16(0x010F); U16(2); U32(mk.Length); U32(data); // texts of 5+ bytes: an offset, never inline
        U16(0x0110); U16(2); U32(md.Length); U32(data + mk.Length);
        U16(0x8769); U16(4); U32(1); U32(exif);
        U32(0);
        U16(1); U16(0x9003); U16(2); U32(wh.Length); U32(data + mk.Length + md.Length); U32(0);
        tiff.AddRange(mk); tiff.AddRange(md); tiff.AddRange(wh);
        byte[] body = [.. "Exif"u8.ToArray(), 0, 0, .. tiff];
        return [0xFF, 0xD8, 0xFF, 0xE1, (byte)((body.Length + 2) >> 8), (byte)(body.Length + 2), .. body, 0xFF, 0xDA, 0, 2, 0, 0];
    }
}
