namespace Nu9ve.Format;

// nu9ve-controls-v1 §3.2 · par de Nu9veFormat NumberText.swift (nu9ve/core). Built digit by digit, without the
// culture's formatter, so every platform writes the same text.

public static class NumberText
{
    /// <summary>U+2212, as wide as a digit in most faces.</summary>
    public const string Minus = "−";
    /// <summary>A no-break space between a number and its unit.</summary>
    public const string UnitSpace = " ";
    public const string NotANumber = "—";
    public static readonly IReadOnlySet<string> TightUnits = new HashSet<string> { "×", "%", "°" };

    /// <summary>Rounded half away from zero to <paramref name="fractionDigits"/>.</summary>
    public static double Rounded(double value, int fractionDigits)
    {
        if (!double.IsFinite(value)) return value;
        var scale = Math.Pow(10, fractionDigits);
        return Math.Round(value * scale, MidpointRounding.AwayFromZero) / scale;
    }

    /// <summary>«−6,0 dB», «440 Hz», «1,25×». No grouping; a rounded zero has no sign.</summary>
    public static string Format(double value, int fractionDigits, string unit = "", string decimalSeparator = ",")
    {
        if (!double.IsFinite(value)) return NotANumber;
        var digits = Math.Max(0, Math.Min(9, fractionDigits));
        var scaled = Math.Round(Math.Abs(value) * Math.Pow(10, digits), MidpointRounding.AwayFromZero);
        if (!(scaled < 9e15)) return NotANumber;
        var units = (ulong)scaled;
        var body = units.ToString(System.Globalization.CultureInfo.InvariantCulture);
        if (digits > 0)
        {
            if (body.Length <= digits) body = new string('0', digits - body.Length + 1) + body;
            body = body[..^digits] + decimalSeparator + body[^digits..];
        }
        var sign = value < 0 && units > 0 ? Minus : "";
        return sign + body + UnitSuffix(unit);
    }

    /// <summary>Decimal units: «999 B», «1,2 KB», «120 GB». One decimal under 10, none from 10 on.</summary>
    public static string Bytes(long count, string decimalSeparator = ",")
    {
        string[] units = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
        var sign = count < 0 ? Minus : "";
        var magnitude = count == long.MinValue ? (double)long.MaxValue : Math.Abs((double)count);
        if (magnitude < 1000) return sign + ((long)magnitude).ToString(System.Globalization.CultureInfo.InvariantCulture) + UnitSpace + "B";
        var index = 0;
        var value = magnitude;
        while (value >= 1000 && index < units.Length - 1) { value /= 1000; index++; }
        var digits = value < 10 ? 1 : 0;
        var shown = Rounded(value, digits);
        if (shown >= 10 && digits == 1) { digits = 0; shown = Rounded(value, 0); }
        if (shown >= 1000 && index < units.Length - 1) { index++; shown = 1; digits = 1; }
        return sign + Format(shown, digits, "", decimalSeparator) + UnitSpace + units[index];
    }

    static string UnitSuffix(string unit) =>
        string.IsNullOrEmpty(unit) ? "" : TightUnits.Contains(unit) ? unit : UnitSpace + unit;
}
