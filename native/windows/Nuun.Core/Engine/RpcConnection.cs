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

    /// <summary>
    /// The Rust engine (`engine/nuun-engine`), same protocol: `nuun-engine [--state-dir …] rpc-serve`.
    /// Found by `NUUN_ENGINE_BIN` or bundled next to the app as `engine\nuun-engine.exe` (nove's layout).
    /// </summary>
    public static string? FindRustEngine()
    {
        var configured = Environment.GetEnvironmentVariable("NUUN_ENGINE_BIN");
        if (!string.IsNullOrEmpty(configured)) return File.Exists(configured) ? configured : null;
        var bundled = Path.Combine(AppContext.BaseDirectory, "engine", "nuun-engine.exe");
        return File.Exists(bundled) ? bundled : null;
    }

    public static (Process Process, RpcConnection Connection) StartRust(string binary, string? stateDirectory)
    {
        var start = new ProcessStartInfo(binary)
        {
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            UseShellExecute = false,
            CreateNoWindow = true,
        };
        if (stateDirectory is not null) { start.ArgumentList.Add("--state-dir"); start.ArgumentList.Add(stateDirectory); }
        start.ArgumentList.Add("rpc-serve");
        var process = Process.Start(start) ?? throw new EngineFailure("runtime_missing", "No se pudo iniciar el motor de Rust");
        return (process, new RpcConnection(process.StandardOutput.BaseStream, process.StandardInput.BaseStream));
    }
}

/// <summary>
/// Writes to the Rust engine: one process per request (`nuun-engine [--state-dir …] rpc`), as the Mac app does with
/// Python, so a write never shares a process with the persistent reader.
/// </summary>
public static class EngineWrites
{
    public static async Task<JsonNode?> CallOnceAsync(string binary, string? stateDirectory, string method, JsonObject parameters,
        CancellationToken cancellation = default)
    {
        var start = new ProcessStartInfo(binary)
        {
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            UseShellExecute = false,
            CreateNoWindow = true,
            StandardOutputEncoding = new UTF8Encoding(false),
        };
        if (stateDirectory is not null) { start.ArgumentList.Add("--state-dir"); start.ArgumentList.Add(stateDirectory); }
        start.ArgumentList.Add("rpc");
        using var process = Process.Start(start) ?? throw new EngineFailure("runtime_missing", "No se pudo iniciar el motor de Rust");
        var request = new JsonObject { ["id"] = 1, ["method"] = method, ["params"] = parameters };
        var bytes = new UTF8Encoding(false).GetBytes(request.ToJsonString());
        await process.StandardInput.BaseStream.WriteAsync(bytes, cancellation).ConfigureAwait(false);
        process.StandardInput.Close();
        var output = await process.StandardOutput.ReadToEndAsync(cancellation).ConfigureAwait(false);
        await process.WaitForExitAsync(cancellation).ConfigureAwait(false);
        JsonNode? envelope;
        try { envelope = JsonNode.Parse(output); }
        catch (JsonException) { throw new EngineFailure("invalid_response", "El motor no respondió JSON"); }
        return RpcConnection.Unwrap(envelope);
    }

    /// <summary>`register_volume`: configuration only (the catalog), never media. External unless <paramref name="allowLocal"/>.</summary>
    public static Task<JsonNode?> RegisterVolumeAsync(string binary, string? stateDirectory, string name, string role, string root,
        string? prefix, bool allowLocal, CancellationToken cancellation = default)
    {
        var parameters = new JsonObject { ["name"] = name, ["role"] = role, ["root"] = root, ["allow_local"] = allowLocal };
        if (!string.IsNullOrWhiteSpace(prefix)) parameters["prefix"] = prefix.Trim();
        return CallOnceAsync(binary, stateDirectory, "register_volume", parameters, cancellation);
    }

    /// <summary>`discover` (read only, one-shot like Python): what a card holds, grouped in units.</summary>
    public static async Task<Discovery> DiscoverAsync(string binary, string? stateDirectory, string source, CancellationToken cancellation = default) =>
        Discovery.Parse(await CallOnceAsync(binary, stateDirectory, "discover", new JsonObject { ["source"] = source }, cancellation).ConfigureAwait(false));

    /// <summary>The engine's codes in words, for the registration dialog.</summary>
    public static string Explain(EngineFailure failure) => failure.Code switch
    {
        "external_required" => "Ese disco no es externo. Si de verdad quieres usar una carpeta interna como destino, marca «Es una carpeta interna».",
        "volume_exists" => "Ya hay un destino con ese nombre o en esa carpeta.",
        "invalid_role" => "El papel debe ser Archivo o Trabajo.",
        "invalid_tag" => "El nombre necesita al menos una letra o un número.",
        "unsafe_path" => "La carpeta dentro del disco debe ser relativa, con «/» y sin «..».",
        "job_busy" => "Otra operación de media está en curso. Intenta de nuevo al terminar.",
        "volume_offline" => "Esa carpeta no está disponible. ¿El disco está conectado?",
        "volume_identity_unavailable" => "Windows no dijo en qué disco físico está ese volumen; nüün no registra un destino sin saberlo.",
        "not_ported" => "Esta versión del motor todavía no hace esto en este sistema.",
        _ => failure.Message,
    };
}

/// <summary>Ends an engine process when its client is disposed.</summary>
public sealed class ProcessOwner(Process process) : IAsyncDisposable
{
    public ValueTask DisposeAsync()
    {
        try { if (!process.HasExited) process.Kill(); } catch (InvalidOperationException) { }
        process.Dispose();
        return ValueTask.CompletedTask;
    }
}
