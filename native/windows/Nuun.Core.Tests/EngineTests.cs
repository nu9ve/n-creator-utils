using System.IO.Pipes;
using System.Text;
using System.Text.Json.Nodes;
using Nuun.Core.Engine;
using Xunit;

namespace Nuun.Core.Tests;

/// <summary>
/// Transport rules shared with Protocol.swift, tested with an in-memory engine. This is not a contract
/// test against media/service.py: on Windows the Python engine is not runnable yet (STATUS).
/// </summary>
public class RpcConnectionTests
{
    sealed class FakeEngine : IAsyncDisposable
    {
        readonly AnonymousPipeServerStream toClient = new(PipeDirection.Out);
        readonly AnonymousPipeServerStream fromClient = new(PipeDirection.In);
        readonly AnonymousPipeClientStream clientIn, clientOut;
        public RpcConnection Connection { get; }
        readonly StreamReader requests;
        readonly StreamWriter responses;

        public FakeEngine()
        {
            clientIn = new AnonymousPipeClientStream(PipeDirection.In, toClient.ClientSafePipeHandle);
            clientOut = new AnonymousPipeClientStream(PipeDirection.Out, fromClient.ClientSafePipeHandle);
            Connection = new RpcConnection(clientIn, clientOut);
            requests = new StreamReader(fromClient, Encoding.UTF8);
            responses = new StreamWriter(toClient, new UTF8Encoding(false)) { AutoFlush = true, NewLine = "\n" };
        }

        public async Task<JsonNode> NextRequestAsync() => JsonNode.Parse((await requests.ReadLineAsync())!)!;
        public Task ReplyAsync(string line) => responses.WriteLineAsync(line);
        public void Close() => toClient.Dispose();

        public async ValueTask DisposeAsync()
        {
            await Connection.DisposeAsync();
            toClient.Dispose(); fromClient.Dispose();
        }
    }

    [Fact]
    public async Task SendsOneLinePerRequestAndUnwrapsData()
    {
        await using var engine = new FakeEngine();
        var call = engine.Connection.CallAsync("snapshot", new JsonObject { ["cards"] = false });
        var request = await engine.NextRequestAsync();
        Assert.Equal("snapshot", request["method"]!.GetValue<string>());
        Assert.False(request["params"]!["cards"]!.GetValue<bool>());
        await engine.ReplyAsync($$$"""{"id":{{{request["id"]}}},"ok":true,"data":{"volumes":[]}}""");
        var data = await call;
        Assert.NotNull(data!["volumes"]);
    }

    [Fact]
    public async Task AnswersMatchTheirRequestOutOfOrder()
    {
        await using var engine = new FakeEngine();
        var first = engine.Connection.CallAsync("volumes");
        var second = engine.Connection.CallAsync("recipe");
        var a = await engine.NextRequestAsync();
        var b = await engine.NextRequestAsync();
        await engine.ReplyAsync($$"""{"id":{{b["id"]}},"ok":true,"data":"recipe"}""");
        await engine.ReplyAsync($$"""{"id":{{a["id"]}},"ok":true,"data":"volumes"}""");
        Assert.Equal("volumes", (await first)!.GetValue<string>());
        Assert.Equal("recipe", (await second)!.GetValue<string>());
    }

    [Fact]
    public async Task EngineErrorsKeepTheirCode()
    {
        await using var engine = new FakeEngine();
        var call = engine.Connection.CallAsync("plan");
        var request = await engine.NextRequestAsync();
        await engine.ReplyAsync($$$"""{"id":{{{request["id"]}}},"ok":false,"error":{"code":"mission_export_missing","message":"Falta el export"}}""");
        var failure = await Assert.ThrowsAsync<EngineFailure>(() => call);
        Assert.Equal("mission_export_missing", failure.Code);
    }

    [Fact]
    public async Task ClosedEngineFailsPendingCalls()
    {
        await using var engine = new FakeEngine();
        var call = engine.Connection.CallAsync("snapshot");
        await engine.NextRequestAsync();
        engine.Close();
        var failure = await Assert.ThrowsAsync<EngineFailure>(() => call);
        Assert.Equal("reader_unavailable", failure.Code);
    }

    [Fact]
    public async Task CancelledCallStopsWaitingAndLateAnswerIsDropped()
    {
        await using var engine = new FakeEngine();
        using var cancel = new CancellationTokenSource();
        var call = engine.Connection.CallAsync("source_cards", null, cancel.Token);
        var request = await engine.NextRequestAsync();
        cancel.Cancel();
        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => call);
        await engine.ReplyAsync($$"""{"id":{{request["id"]}},"ok":true,"data":[]}""");
        var next = engine.Connection.CallAsync("volumes");
        var nextRequest = await engine.NextRequestAsync();
        await engine.ReplyAsync($$"""{"id":{{nextRequest["id"]}},"ok":true,"data":1}""");
        Assert.Equal(1, (await next)!.GetValue<int>());
    }

    [Fact]
    public void EnvelopeRuleMatchesSwift()
    {
        Assert.Equal(3, RpcConnection.Unwrap(JsonNode.Parse("""{"ok":true,"data":3}"""))!.GetValue<int>());
        Assert.Throws<EngineFailure>(() => RpcConnection.Unwrap(JsonNode.Parse("""{"data":3}""")));
        Assert.Throws<EngineFailure>(() => RpcConnection.Unwrap(JsonNode.Parse("[1]")));
    }
}

public class FixtureEngineTests
{
    [Fact]
    public async Task SnapshotHasTheEngineShape()
    {
        await using var engine = new FixtureEngine();
        var snapshot = await engine.SnapshotAsync();
        Assert.True(engine.IsFixture);
        Assert.Equal(3, snapshot.Volumes.Count);
        Assert.Contains(snapshot.Volumes, v => v.Role == VolumeRole.Archive && v.Online);
        Assert.Equal("sony", snapshot.Cards[0].Family);
        Assert.Equal("sony_format_in_camera", snapshot.Cards[0].Recommendation);
        // Fixture names only: never personal disks (AGENTS.md).
        Assert.All(snapshot.Volumes, v => Assert.EndsWith("-DEMO", v.Name));
        Assert.All(snapshot.Projects, p => Assert.EndsWith("-demo", p.Name));
    }

    [Fact]
    public async Task RecipeListsTheCardFamilies()
    {
        await using var engine = new FixtureEngine();
        var families = (await engine.RecipeAsync()).Select(c => c.Family).ToArray();
        Assert.Equal(["stalker", "sony", "h6", "gopro", "insta360", "dji", "canon"], families);
    }

    [Fact]
    public async Task ApplyReportsOneOverallMeasureWithALog()
    {
        // Same rule as tests/test_action_progress.py: one measure that never goes back, ends at its total.
        await using var engine = new FixtureEngine(TimeSpan.Zero);
        var snapshot = await engine.SnapshotAsync();
        var plan = await engine.PlanAsync(snapshot.Cards[0], "proyecto-demo");
        var reports = new List<ActionProgress>();
        await engine.ApplyAsync(plan, new SyncProgress(reports.Add));
        Assert.NotEmpty(reports);
        for (var i = 1; i < reports.Count; i++) Assert.True(reports[i].OverallDone >= reports[i - 1].OverallDone);
        var last = reports[^1];
        Assert.True(last.Finished);
        Assert.Equal(last.OverallTotal, last.OverallDone);
        Assert.Contains(last.Log, line => line.StartsWith("✓", StringComparison.Ordinal));
        Assert.All(reports.Where(r => !r.Finished), r => Assert.StartsWith("Copiando", r.Verb));
    }

    sealed class SyncProgress(Action<ActionProgress> report) : IProgress<ActionProgress>
    {
        public void Report(ActionProgress value) => report(value);
    }
}
