using System.Collections.Concurrent;
using System.Diagnostics;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace Nuun.Core.Engine;

/// <summary>A failure the engine reported (`{"ok": false, "error": {code, message}}`) or the transport hit.</summary>
public sealed class EngineFailure(string code, string message) : Exception(message)
{
    public string Code { get; } = code;
}

// Mismo protocolo que native/MediaMenu/Sources/MediaMenuCore/Protocol.swift (PersistentBackend):
// una línea JSON por petición `{"id", "method", "params"}` y una por respuesta, con el sobre
// `{"ok": true, "data": …}` o `{"ok": false, "error": {"code", "message"}}` (ProtocolBoundary.response).
// El transporte es independiente del proceso para poder probarlo con flujos en memoria.
/// <summary>Line-delimited JSON RPC over a pair of streams, with id correlation and cancellation.</summary>
public sealed class RpcConnection : IAsyncDisposable
{
    readonly Stream output;
    readonly StreamReader input;
    readonly ConcurrentDictionary<long, TaskCompletionSource<JsonNode?>> pending = new();
    readonly SemaphoreSlim writeLock = new(1, 1);
    readonly CancellationTokenSource stop = new();
    readonly Task reader;
    long nextId;

    public RpcConnection(Stream fromEngine, Stream toEngine)
    {
        input = new StreamReader(fromEngine, new UTF8Encoding(false));
        output = toEngine;
        reader = Task.Run(ReadLoopAsync);
    }

    /// <summary>The envelope rule shared with Swift: data on success, an <see cref="EngineFailure"/> otherwise.</summary>
    public static JsonNode? Unwrap(JsonNode? envelope)
    {
        if (envelope is not JsonObject obj) throw new EngineFailure("invalid_response", "Respuesta sin sobre");
        if (obj["ok"] is JsonValue ok && ok.TryGetValue<bool>(out var success) && success) return obj["data"];
        var error = obj["error"];
        throw new EngineFailure(error?["code"]?.GetValue<string>() ?? "engine_error", error?["message"]?.GetValue<string>() ?? "Error del motor");
    }

    public async Task<JsonNode?> CallAsync(string method, JsonObject? parameters = null, CancellationToken cancellation = default)
    {
        var id = Interlocked.Increment(ref nextId);
        var completion = new TaskCompletionSource<JsonNode?>(TaskCreationOptions.RunContinuationsAsynchronously);
        pending[id] = completion;
        var request = new JsonObject { ["id"] = id, ["method"] = method, ["params"] = parameters ?? new JsonObject() };
        var line = Encoding.UTF8.GetBytes(request.ToJsonString() + "\n");
        await writeLock.WaitAsync(cancellation).ConfigureAwait(false);
        try
        {
            await output.WriteAsync(line, cancellation).ConfigureAwait(false);
            await output.FlushAsync(cancellation).ConfigureAwait(false);
        }
        catch (Exception error) when (error is IOException or ObjectDisposedException)
        {
            pending.TryRemove(id, out _);
            throw new EngineFailure("reader_unavailable", error.Message);
        }
        finally { writeLock.Release(); }
        // A cancelled caller stops waiting; a late answer for its id is dropped by the reader.
        using var registration = cancellation.Register(() =>
        {
            if (pending.TryRemove(id, out var waiting)) waiting.TrySetCanceled(cancellation);
        });
        return await completion.Task.ConfigureAwait(false);
    }

    async Task ReadLoopAsync()
    {
        try
        {
            while (!stop.IsCancellationRequested)
            {
                var line = await input.ReadLineAsync(stop.Token).ConfigureAwait(false);
                if (line is null) break;
                JsonNode? envelope;
                try { envelope = JsonNode.Parse(line); } catch (JsonException) { continue; }
                if (envelope?["id"] is not JsonValue idValue || !idValue.TryGetValue<long>(out var id)) continue;
                if (!pending.TryRemove(id, out var waiting)) continue;
                try { waiting.TrySetResult(Unwrap(envelope)); }
                catch (EngineFailure failure) { waiting.TrySetException(failure); }
            }
        }
        catch (Exception error) when (error is OperationCanceledException or IOException or ObjectDisposedException) { }
        finally
        {
            foreach (var key in pending.Keys)
                if (pending.TryRemove(key, out var waiting))
                    waiting.TrySetException(new EngineFailure("reader_unavailable", "Lector reiniciado"));
        }
    }

    public async ValueTask DisposeAsync()
    {
        stop.Cancel();
        try { await output.DisposeAsync().ConfigureAwait(false); } catch (IOException) { }
        input.Dispose();
        try { await reader.ConfigureAwait(false); } catch (OperationCanceledException) { }
        stop.Dispose();
        writeLock.Dispose();
    }
}

/// <summary>Where the Python engine lives. Windows has no packaged runtime yet (STATUS: Distribución).</summary>
public sealed record EngineConfiguration(string Python, string Script, string? StateDirectory)
{
    /// <summary>`NUUN_PYTHON` and `NUUN_ENGINE` (path to nu9ve.py); null when either is missing.</summary>
    public static EngineConfiguration? FromEnvironment()
    {
        var python = Environment.GetEnvironmentVariable("NUUN_PYTHON");
        var script = Environment.GetEnvironmentVariable("NUUN_ENGINE");
        if (string.IsNullOrEmpty(python) || string.IsNullOrEmpty(script) || !File.Exists(script)) return null;
        return new(python, script, Environment.GetEnvironmentVariable("NUUN_QA_ROOT"));
    }
}

/// <summary>Starts `python -S nu9ve.py media [--state-dir …] rpc-serve` without a console window.</summary>
public static class EngineProcess
{
    public static (Process Process, RpcConnection Connection) Start(EngineConfiguration configuration)
    {
        var start = new ProcessStartInfo(configuration.Python)
        {
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = false,
            UseShellExecute = false,
            CreateNoWindow = true,
        };
        foreach (var argument in new[] { "-S", configuration.Script, "media" }) start.ArgumentList.Add(argument);
        if (configuration.StateDirectory is { } state) { start.ArgumentList.Add("--state-dir"); start.ArgumentList.Add(state); }
        start.ArgumentList.Add("rpc-serve");
        var process = Process.Start(start) ?? throw new EngineFailure("runtime_missing", "No se pudo iniciar el motor");
        return (process, new RpcConnection(process.StandardOutput.BaseStream, process.StandardInput.BaseStream));
    }
}
