namespace Nu9ve.Format;

// nu9ve-controls-v1 §4 · par de Nu9veFormat Instant.swift (nu9ve/core). Parsed by hand, not with DateTime.Parse,
// so every platform reads the same instants.

public static class Instant
{
    /// <summary>
    /// Seconds since 1970-01-01T00:00:00Z of an ISO 8601 instant, or null. Accepts «YYYY-MM-DDTHH:MM», with «:SS» and a
    /// fraction of 1 to 9 digits, «T» or a space, and a zone «Z», «±HH:MM» or «±HHMM».
    /// </summary>
    public static double? Seconds(string raw)
    {
        var t = raw;
        var i = 0;
        int? Number(int width)
        {
            if (i + width > t.Length) return null;
            var value = 0;
            for (var k = i; k < i + width; k++)
            {
                if (t[k] < '0' || t[k] > '9') return null;
                value = value * 10 + (t[k] - '0');
            }
            i += width;
            return value;
        }
        bool Take(char c)
        {
            if (i >= t.Length || t[i] != c) return false;
            i++;
            return true;
        }
        if (Number(4) is not int year || !Take('-') || Number(2) is not int month || !Take('-') || Number(2) is not int day) return null;
        if (!(Take('T') || Take(' ')) || Number(2) is not int hour || !Take(':') || Number(2) is not int minute) return null;
        var second = 0;
        var fraction = 0.0;
        if (Take(':'))
        {
            if (Number(2) is not int s) return null;
            second = s;
            if (Take('.'))
            {
                var digits = 0;
                var value = 0.0;
                var scale = 1.0;
                while (i < t.Length && t[i] >= '0' && t[i] <= '9')
                {
                    scale /= 10;
                    value += (t[i] - '0') * scale;
                    digits++;
                    i++;
                }
                if (digits < 1 || digits > 9) return null;
                fraction = value;
            }
        }
        var offset = 0;
        if (Take('Z')) offset = 0;
        else if (i < t.Length && (t[i] == '+' || t[i] == '-'))
        {
            var sign = t[i] == '-' ? -1 : 1;
            i++;
            if (Number(2) is not int oh) return null;
            Take(':');
            if (Number(2) is not int om || oh > 23 || om > 59) return null;
            offset = sign * (oh * 3600 + om * 60);
        }
        else return null;
        if (i != t.Length) return null;
        if (month < 1 || month > 12 || day < 1 || day > DaysIn(month, year) || hour > 23 || minute > 59 || second > 59) return null;
        long whole = DaysFromCivil(year, month, day) * 86_400L + hour * 3600 + minute * 60 + second - offset;
        return whole + fraction;
    }

    public static DateTimeOffset? Date(string raw) =>
        Seconds(raw) is double s ? DateTimeOffset.UnixEpoch.AddTicks((long)Math.Round(s * TimeSpan.TicksPerSecond)) : null;

    /// <summary>«YYYY-MM-DD HH:MM» on the wall clock of a zone (IANA id), the same on every platform.</summary>
    public static string WallClock(double seconds, TimeZoneInfo zone)
    {
        var utc = DateTimeOffset.UnixEpoch.AddTicks((long)Math.Floor(seconds * TimeSpan.TicksPerSecond));
        var local = TimeZoneInfo.ConvertTime(utc, zone);
        return local.ToString("yyyy-MM-dd HH:mm", System.Globalization.CultureInfo.InvariantCulture);
    }

    /// <summary>What a person reads, in their zone and culture; what cannot be read is shown as it came.</summary>
    public static string Label(string raw, string format = "g", System.Globalization.CultureInfo? culture = null, TimeZoneInfo? zone = null)
    {
        if (Date(raw) is not DateTimeOffset date) return raw;
        return TimeZoneInfo.ConvertTime(date, zone ?? TimeZoneInfo.Local).ToString(format, culture ?? System.Globalization.CultureInfo.CurrentCulture);
    }

    internal static int DaysIn(int month, int year) => month switch
    {
        2 => (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 ? 29 : 28,
        4 or 6 or 9 or 11 => 30,
        _ => 31,
    };

    internal static long DaysFromCivil(int year, int month, int day)
    {
        long y = month <= 2 ? year - 1 : year;
        var era = (y >= 0 ? y : y - 399) / 400;
        var yoe = y - era * 400;
        var mp = (month + 9) % 12;
        var doy = (153 * mp + 2) / 5 + day - 1;
        var doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        return era * 146_097 + doe - 719_468;
    }
}

/// <summary>A range of instants: the start included, the end excluded.</summary>
public readonly record struct InstantRange(double Start, double End)
{
    public bool IsValid => double.IsFinite(Start) && double.IsFinite(End) && Start < End;
    public bool Contains(double seconds) => seconds >= Start && seconds < End;
}
