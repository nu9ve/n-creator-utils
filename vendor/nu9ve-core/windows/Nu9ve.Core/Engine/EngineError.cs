using System.Text.Json.Nodes;

namespace Nu9ve.Engine;

/// <summary>
/// A failed call (nu9ve-engine-v1 §7): an error the engine answered, with the method's own code
/// (<c>project_not_found</c>), or one this client produced (<see cref="EngineErrorCodes"/>, never sent by an engine).
/// Same shape as <c>EngineError</c> in Swift's <c>Nu9veEngine</c>.
/// </summary>
public sealed class EngineException : Exception
{
    /// <summary>What the program compares: <c>^[a-z_]{1,64}$</c>.</summary>
    public string Code { get; }

    /// <summary>The engine wrote it; false when this client produced it.</summary>
    public bool FromEngine { get; }

    /// <summary>
    /// The request reached the engine whole. After a timeout or a lost connection with this set, a write may have
    /// happened: ask its status, do not resend it blindly (§6.3).
    /// </summary>
    public bool Dispatched { get; }

    /// <summary>The engine's whole <c>error</c> object, with <c>reason</c>, <c>retryable</c> and the app's own fields.</summary>
    public JsonObject Details { get; }

    /// <summary>Repeating the same request may work (§3). False when the engine did not say.</summary>
    public bool Retryable => Details["retryable"] is JsonValue v && v.TryGetValue(out bool b) && b;

    /// <summary>How long to wait before retrying, when the engine said it (§3).</summary>
    public int? RetryAfterSeconds => Details["retry_after_seconds"] is JsonValue v && v.TryGetValue(out int s) ? s : null;

    /// <summary>Why it is <c>not_ported</c> (§5), or another code's own reason.</summary>
    public string? Reason => Details["reason"] is JsonValue v && v.TryGetValue(out string? r) ? r : null;

    EngineException(string code, string message, bool fromEngine, bool dispatched, JsonObject details)
        : base(message)
    {
        Code = code;
        FromEngine = fromEngine;
        Dispatched = dispatched;
        Details = details;
    }

    /// <summary>From the <c>error</c> of an envelope <see cref="EngineEnvelope.TryParse(string, out EngineEnvelope?, out EnvelopeProblem)"/> already checked.</summary>
    public static EngineException FromEngineError(JsonObject error)
    {
        var copy = (JsonObject)error.DeepClone();
        var code = copy["code"]?.GetValue<string>() ?? EngineErrorCodes.InternalError;
        var message = copy["message"]?.GetValue<string>() ?? "";
        return new EngineException(code, message, fromEngine: true, dispatched: true, copy);
    }

    /// <summary>One this client produced: <see cref="EngineErrorCodes.Timeout"/>, <see cref="EngineErrorCodes.ConnectionLost"/>…</summary>
    public static EngineException FromClient(string code, string message, bool dispatched = false, bool retryable = false)
    {
        var details = new JsonObject { ["code"] = code, ["message"] = message };
        if (retryable) details["retryable"] = true;
        return new EngineException(code, message, fromEngine: false, dispatched, details);
    }

    public override string ToString() => $"{Code}: {Message}";
}

/// <summary>The codes of nu9ve-engine-v1 §7. Each app adds those of its domain.</summary>
public static class EngineErrorCodes
{
    // Written by the engine.
    public const string InvalidRequest = "invalid_request";
    public const string InvalidParams = "invalid_params";
    public const string RequestTooLarge = "request_too_large";
    public const string ResponseTooLarge = "response_too_large";
    public const string UnknownMethod = "unknown_method";
    public const string NotPorted = "not_ported";
    public const string ReadOnly = "read_only";
    public const string ProtocolVersion = "protocol_version";
    public const string EngineBusy = "engine_busy";
    public const string ResponseTimeout = "response_timeout";
    public const string IdempotencyKey = "idempotency_key";
    public const string Cancelled = "cancelled";
    public const string InternalError = "internal_error";

    // Produced by the client, never by an engine.
    public const string EngineMissing = "engine_missing";
    public const string StartupFailed = "startup_failed";
    public const string StartupTimeout = "startup_timeout";
    public const string IncompatibleEngine = "incompatible_engine";
    public const string Timeout = "timeout";
    public const string ConnectionLost = "connection_lost";
    /// <summary>A socket or pipe that does not answer.</summary>
    public const string EngineUnavailable = "engine_unavailable";
    /// <summary>The peer of the socket or pipe is another user (§9).</summary>
    public const string PeerUser = "peer_user";
    /// <summary>The socket or pipe does not pass the checks of §9 before connecting.</summary>
    public const string UnsafeEndpoint = "unsafe_endpoint";
    /// <summary>The engine broke the protocol: an invalid envelope or an id nobody asked for (§2.1).</summary>
    public const string InvalidResponse = "invalid_response";
    /// <summary>A lease released after it had already ended (§6.4).</summary>
    public const string LeaseEnded = "lease_ended";
}
