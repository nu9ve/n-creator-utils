using System.Globalization;

namespace Nu9ve.Theme;

// nu9ve-theme-v1 §3 · origen: n-creator-utils native/MediaMenu/Sources/MediaMenuCore/ThemeColor.swift
// (ThemeColor), que a su vez viene de soundshop Appearance.swift. Port literal: mismas fórmulas,
// mismo orden de operaciones y mismo redondeo de hex (al más cercano, lejos de cero, como Swift),
// para que los vectores compartidos den el mismo hex en Swift y en C#.
/// <summary>Opaque, normalized sRGB. Pure so the palette can be tested without WinUI.</summary>
public readonly record struct ThemeColor(double Red, double Green, double Blue)
{
    public static readonly ThemeColor Black = new(0, 0, 0);
    public static readonly ThemeColor White = new(1, 1, 1);

    public static ThemeColor? FromHex(string? hex)
    {
        if (hex is null) return null;
        var value = hex.Trim();
        if (value.StartsWith('#')) value = value[1..];
        if (value.Length != 6 || !value.All(Uri.IsHexDigit)) return null;
        var rgb = uint.Parse(value, NumberStyles.HexNumber, CultureInfo.InvariantCulture);
        return new ThemeColor(((rgb >> 16) & 255) / 255.0, ((rgb >> 8) & 255) / 255.0, (rgb & 255) / 255.0);
    }

    /// <summary>Parses a hex that is known to be valid (presets and skin seeds).</summary>
    public static ThemeColor Hex(string hex) => FromHex(hex) ?? throw new ArgumentException($"Invalid hex {hex}");

    static int Byte(double channel) => (int)Math.Round(channel * 255, MidpointRounding.AwayFromZero);

    public string ToHex() => $"#{Byte(Red):X2}{Byte(Green):X2}{Byte(Blue):X2}";

    /// <summary>(R, G, B) bytes for platform colors.</summary>
    public (byte R, byte G, byte B) Bytes =>
        ((byte)Math.Clamp(Byte(Red), 0, 255), (byte)Math.Clamp(Byte(Green), 0, 255), (byte)Math.Clamp(Byte(Blue), 0, 255));

    /// <summary>WCAG relative luminance.</summary>
    public double Luminance
    {
        get
        {
            static double Linear(double x) => x <= 0.04045 ? x / 12.92 : Math.Pow((x + 0.055) / 1.055, 2.4);
            return 0.2126 * Linear(Red) + 0.7152 * Linear(Green) + 0.0722 * Linear(Blue);
        }
    }

    public double Contrast(ThemeColor other) =>
        (Math.Max(Luminance, other.Luminance) + 0.05) / (Math.Min(Luminance, other.Luminance) + 0.05);

    public ThemeColor Mixed(ThemeColor other, double amount) =>
        new(Red + (other.Red - Red) * amount, Green + (other.Green - Green) * amount, Blue + (other.Blue - Blue) * amount);

    /// <summary>Keep the color unchanged when it already reads well; otherwise mix toward the best text polarity.</summary>
    public ThemeColor Readable(IReadOnlyList<ThemeColor> backgrounds, double minimum = 4.5)
    {
        double Score(ThemeColor color) => backgrounds.Count == 0 ? 21 : backgrounds.Min(color.Contrast);
        if (Score(this) >= minimum) return this;
        var target = Score(Black) >= Score(White) ? Black : White;
        if (Score(target) < minimum) return target;
        double low = 0, high = 1;
        for (var i = 0; i < 20; i++)
        {
            var mid = (low + high) / 2;
            if (Score(Mixed(target, mid)) >= minimum) high = mid; else low = mid;
        }
        return Mixed(target, high);
    }

    /// <summary>Black or white, whichever reads best on this color.</summary>
    public ThemeColor OnColor => Contrast(Black) >= Contrast(White) ? Black : White;

    /// <summary>Hue, saturation and value in 0…1.</summary>
    public (double Hue, double Saturation, double Value) Hsv
    {
        get
        {
            double maxC = Math.Max(Red, Math.Max(Green, Blue)), minC = Math.Min(Red, Math.Min(Green, Blue)), delta = maxC - minC;
            var hue = 0.0;
            if (delta > 0)
            {
                // Swift truncatingRemainder == C# % for doubles (sign of the dividend).
                if (maxC == Red) hue = ((Green - Blue) / delta) % 6;
                else if (maxC == Green) hue = (Blue - Red) / delta + 2;
                else hue = (Red - Green) / delta + 4;
                hue /= 6;
                if (hue < 0) hue += 1;
            }
            return (hue, maxC == 0 ? 0 : delta / maxC, maxC);
        }
    }

    public static ThemeColor FromHsv(double hue, double saturation, double value)
    {
        var h = (hue >= 1 ? 0 : Math.Max(0, hue)) * 6;
        var sector = (int)h;
        var fraction = h - Math.Floor(h);
        double p = value * (1 - saturation), q = value * (1 - saturation * fraction), t = value * (1 - saturation * (1 - fraction));
        return sector switch
        {
            0 => new(value, t, p),
            1 => new(q, value, p),
            2 => new(p, value, t),
            3 => new(p, q, value),
            4 => new(t, p, value),
            _ => new(value, p, q),
        };
    }

    public override string ToString() => ToHex();
}
