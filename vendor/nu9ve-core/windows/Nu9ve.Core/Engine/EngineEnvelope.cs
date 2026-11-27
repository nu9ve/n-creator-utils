using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace Nu9ve.Engine;

/// <summary>
/// A request or response id (nu9ve-engine-v1 §2.1): an integer as written, a string, or null. <c>7</c> and
/// <c>"7"</c> are different ids, and so are <c>7</c> and <c>7.0</c> (which is not an id). Integers keep their digits,
/// so one beyond 64 bits still matches exactly.
/// </summary>
public readonly record struct EngineId
{
    public enum IdKind { Null, Integer, String }

    public IdKind Kind { get; }

    /// <summary>The digits of an integer or the text of a string; empty for null.</summary>
    public string Text { get; }

    EngineId(IdKind kind, string text)
    {
        Kind = kind;
        Text = text;
    }

    public static EngineId Null => new(IdKind.Null, "");
    public static EngineId Of(long value) => new(IdKind.Integer, value.ToString(System.Globalization.CultureInfo.InvariantCulture));
    public static EngineId Of(string value) => new(IdKind.String, value);

    /// <summary>Null for a float, a bool, an object or an array.</summary>
    public static EngineId? FromJson(JsonElement value)
    {
        switch (value.ValueKind)
        {
            case JsonValueKind.Null:
                return Null;
            case JsonValueKind.String:
                return Of(value.GetString()!);
            case JsonValueKind.Number:
                var raw = value.GetRawText();
                if (raw.AsSpan().IndexOfAny('.', 'e', 'E') >= 0) return null;
                return new EngineId(IdKind.Integer, raw == "-0" ? "0" : raw);
            default:
                return null;
        }
    }

    /// <summary>An id written as JSON text (<c>7</c>, <c>"7"</c>, <c>null</c>), or null when it is not an id.</summary>
    public static EngineId? Parse(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            return FromJson(doc.RootElement);
        }
        catch (JsonException)
        {
            return null;
        }
    }

    /// <summary>How it goes back into a request: JSON text.</summary>
    public override string ToString() => Kind switch
    {
        IdKind.Null => "null",
        IdKind.Integer => Text,
        _ => JsonSerializer.Serialize(Text),
    };
}

/// <summary>Why a response envelope is not valid §3. <see cref="EngineEnvelope.Why"/> gives the <c>why</c> of the examples.</summary>
public enum EnvelopeProblem
{
    None,
    NotJson,
    NotObject,
    BadId,
    BadOk,
    DataAndError,
    BadCode,
    BadMessage,
    NotPortedWithoutReason,
}

/// <summary>
/// A response the engine wrote, checked against nu9ve-engine-v1 §3: <c>id</c>, a boolean <c>ok</c> and exactly one of
/// <c>data</c> or <c>error</c>, with <c>error.code</c> as <c>^[a-z_]{1,64}$</c>, a string <c>error.message</c>, and
/// <c>reason</c> (<c>^[a-z_]{1,40}$</c>) when the code is <c>not_ported</c>. Fields it does not know are ignored.
/// The transport stays in each app; this is what comes after it.
/// </summary>
public sealed class EngineEnvelope
{
    public EngineId Id { get; }
    public bool Ok { get; }

    /// <summary>The value of <c>data</c> when <see cref="Ok"/>; it may be JSON null.</summary>
    public JsonNode? Data { get; }

    /// <summary>The whole <c>error</c> object when not <see cref="Ok"/>.</summary>
    public JsonObject? Error { get; }

    EngineEnvelope(EngineId id, bool ok, JsonNode? data, JsonObject? error)
    {
        Id = id;
        Ok = ok;
        Data = data;
        Error = error;
    }

    public static bool TryParse(string text, out EngineEnvelope? envelope, out EnvelopeProblem problem) =>
        TryParse(Encoding.UTF8.GetBytes(text), out envelope, out problem);

    public static bool TryParse(ReadOnlyMemory<byte> utf8, out EngineEnvelope? envelope, out EnvelopeProblem problem)
    {
        envelope = null;
        problem = Check(utf8, ref envelope);
        return problem == EnvelopeProblem.None;
    }

    static EnvelopeProblem Check(ReadOnlyMemory<byte> utf8, ref EngineEnvelope? envelope)
    {
        JsonDocument doc;
        try
        {
            doc = JsonDocument.Parse(utf8);
        }
        catch (JsonException)
        {
            return EnvelopeProblem.NotJson;
        }
        using (doc)
        {
            var root = doc.RootElement;
            if (root.ValueKind != JsonValueKind.Object) return EnvelopeProblem.NotObject;
            if (!root.TryGetProperty("id", out var rawId) || EngineId.FromJson(rawId) is not { } id) return EnvelopeProblem.BadId;
            if (!root.TryGetProperty("ok", out var ok) || ok.ValueKind is not (JsonValueKind.True or JsonValueKind.False))
                return EnvelopeProblem.BadOk;
            var hasData = root.TryGetProperty("data", out var data);
            var hasError = root.TryGetProperty("error", out var error);
            if (ok.GetBoolean())
            {
                if (!hasData || hasError) return EnvelopeProblem.DataAndError;
                envelope = new EngineEnvelope(id, true, JsonNode.Parse(data.GetRawText()), null);
                return EnvelopeProblem.None;
            }
            if (hasData || !hasError || error.ValueKind != JsonValueKind.Object) return EnvelopeProblem.DataAndError;
            if (!error.TryGetProperty("code", out var code) || code.ValueKind != JsonValueKind.String || !IsToken(code.GetString()!, 64))
                return EnvelopeProblem.BadCode;
            if (!error.TryGetProperty("message", out var message) || message.ValueKind != JsonValueKind.String)
                return EnvelopeProblem.BadMessage;
            if (code.GetString() == EngineErrorCodes.NotPorted
                && (!error.TryGetProperty("reason", out var reason) || reason.ValueKind != JsonValueKind.String || !IsToken(reason.GetString()!, 40)))
                return EnvelopeProblem.NotPortedWithoutReason;
            envelope = new EngineEnvelope(id, false, null, JsonNode.Parse(error.GetRawText())!.AsObject());
            return EnvelopeProblem.None;
        }
    }

    /// <summary>
    /// The <c>data</c> of a response to the request with <paramref name="expected"/>, or the engine's error as an
    /// <see cref="EngineException"/>. An invalid envelope or another id is <see cref="EngineErrorCodes.InvalidResponse"/>:
    /// the client closes that process (§2.1).
    /// </summary>
    public static JsonNode? Unwrap(ReadOnlyMemory<byte> utf8, EngineId expected)
    {
        if (!TryParse(utf8, out var envelope, out var problem))
            throw EngineException.FromClient(EngineErrorCodes.InvalidResponse, $"Invalid engine response: {Why(problem)}.", dispatched: true);
        if (envelope!.Id != expected)
            throw EngineException.FromClient(EngineErrorCodes.InvalidResponse, $"Response for id {envelope.Id}, expected {expected}.", dispatched: true);
        return envelope.Ok ? envelope.Data : throw EngineException.FromEngineError(envelope.Error!);
    }

    /// <summary><c>^[a-z_]{1,limit}$</c>, the shape of <c>error.code</c> (64) and <c>error.reason</c> (40).</summary>
    public static bool IsToken(string text, int limit) =>
        text.Length > 0 && text.Length <= limit && text.All(c => c is (>= 'a' and <= 'z') or '_');

    /// <summary>The <c>why</c> of <c>data/engine-examples-v1.json</c>.</summary>
    public static string Why(EnvelopeProblem problem) => problem switch
    {
        EnvelopeProblem.None => "",
        EnvelopeProblem.NotJson => "not_json",
        EnvelopeProblem.NotObject => "not_object",
        EnvelopeProblem.BadId => "bad_id",
        EnvelopeProblem.BadOk => "bad_ok",
        EnvelopeProblem.DataAndError => "data_and_error",
        EnvelopeProblem.BadCode => "bad_code",
        EnvelopeProblem.BadMessage => "bad_message",
        EnvelopeProblem.NotPortedWithoutReason => "not_ported_without_reason",
        _ => throw new ArgumentOutOfRangeException(nameof(problem)),
    };
}
