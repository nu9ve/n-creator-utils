namespace Nu9ve.Format;

// nu9ve-controls-v1 §4 · par de Nu9veFormat Timecode.swift (nu9ve/core).

public static class Timecode
{
    /// <summary>«m:ss.cc», or «h:mm:ss.cc» from an hour on; «--:--.--» when not finite.</summary>
    public static string Format(double seconds)
    {
        if (!double.IsFinite(seconds)) return "--:--.--";
        var hundredths = Math.Round(Math.Abs(seconds) * 100, MidpointRounding.AwayFromZero);
        if (!(hundredths < 9e15)) return "--:--.--";
        var total = (ulong)hundredths;
        ulong cc = total % 100, s = total / 100 % 60, m = total / 6000 % 60, h = total / 360_000;
        var body = h > 0 ? $"{h}:{Pad(m, 2)}:{Pad(s, 2)}.{Pad(cc, 2)}" : $"{m}:{Pad(s, 2)}.{Pad(cc, 2)}";
        return (seconds < 0 && total > 0 ? NumberText.Minus : "") + body;
    }

    /// <summary>«h:mm:ss:ff» with a whole frame rate: the frame the time falls in.</summary>
    public static string Format(double seconds, int framesPerSecond)
    {
        if (!double.IsFinite(seconds) || framesPerSecond < 1) return "--:--:--:--";
        var frames = Math.Floor(Math.Abs(seconds) * framesPerSecond + 1e-9);
        if (!(frames < 9e15)) return "--:--:--:--";
        ulong total = (ulong)frames, fps = (ulong)framesPerSecond;
        ulong ff = total % fps, whole = total / fps;
        ulong s = whole % 60, m = whole / 60 % 60, h = whole / 3600;
        var body = $"{h}:{Pad(m, 2)}:{Pad(s, 2)}:{Pad(ff, (fps - 1).ToString(System.Globalization.CultureInfo.InvariantCulture).Length)}";
        return (seconds < 0 && total > 0 ? NumberText.Minus : "") + body;
    }

    static string Pad(ulong value, int width) => value.ToString(System.Globalization.CultureInfo.InvariantCulture).PadLeft(width, '0');
}
