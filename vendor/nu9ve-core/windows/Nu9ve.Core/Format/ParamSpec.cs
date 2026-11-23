namespace Nu9ve.Format;

// nu9ve-controls-v1 §3 · par de Nu9veFormat ParamSpec.swift (nu9ve/core). Los dos se prueban con
// data/ui-format-vectors-v1.json.

/// <summary>How a value maps to the travel of a control. <see cref="Log"/> needs <c>Min &gt; 0</c>.</summary>
public enum ValueScale { Linear, Log }

/// <summary>One step: the arrows give <see cref="Normal"/>, Shift (or Page Up/Down) <see cref="Large"/>, Alt <see cref="Fine"/>.</summary>
public enum ParamStep { Fine, Normal, Large }

public static class ParamStepInfo
{
    public static string Id(this ParamStep step) => step switch { ParamStep.Fine => "fine", ParamStep.Large => "large", _ => "normal" };

    public static ParamStep? FromId(string? id) => id switch
    {
        "fine" => ParamStep.Fine,
        "normal" => ParamStep.Normal,
        "large" => ParamStep.Large,
        _ => null,
    };
}

/// <summary>
/// A value a person can adjust, as an app or its engine declares it. <see cref="Step"/> is in the value's units with a
/// linear scale; with a log scale it is a fraction of the travel. A large step is <see cref="LargeSteps"/> steps, a
/// fine one a tenth of one.
/// </summary>
public sealed record ParamSpec(
    string Id,
    string Label,
    double Min,
    double Max,
    double Step,
    double DefaultValue,
    double? OriginValue = null,
    ValueScale Scale = ValueScale.Linear,
    string Unit = "",
    int FractionDigits = 0,
    double LargeSteps = 4)
{
    /// <summary>Where the filled part of the track starts: <see cref="Min"/> when not given.</summary>
    public double Origin => OriginValue ?? Min;

    public bool IsValid
    {
        get
        {
            if (!double.IsFinite(Min) || !double.IsFinite(Max) || !(Min < Max) || !double.IsFinite(Step) || !(Step > 0)
                || LargeSteps < 1 || FractionDigits < 0 || FractionDigits > 9) return false;
            if (DefaultValue < Min || DefaultValue > Max || Origin < Min || Origin > Max) return false;
            return Scale != ValueScale.Log || (Min > 0 && Step <= 1);
        }
    }

    /// <summary>The value inside the bounds. Not a number becomes the default.</summary>
    public double Clamped(double value) => double.IsNaN(value) ? DefaultValue : Math.Min(Max, Math.Max(Min, value));

    public double PositionOf(double value)
    {
        var v = Clamped(value);
        return Scale == ValueScale.Linear ? (v - Min) / (Max - Min) : Math.Log(v / Min) / Math.Log(Max / Min);
    }

    public double ValueAt(double position)
    {
        var t = double.IsNaN(position) ? PositionOf(DefaultValue) : Math.Min(1, Math.Max(0, position));
        return Scale == ValueScale.Linear ? Clamped(Min + t * (Max - Min)) : Clamped(Min * Math.Pow(Max / Min, t));
    }

    public double Snapped(double value)
    {
        var v = Clamped(value);
        if (Scale == ValueScale.Log) return Clamped(NumberText.Rounded(v, FractionDigits));
        var steps = Math.Round((v - Min) / Step, MidpointRounding.AwayFromZero);
        return Clamped(Min + steps * Step);
    }

    /// <summary>The value after <paramref name="count"/> steps (negative goes down). A fine step is not snapped.</summary>
    public double Stepped(double value, ParamStep kind = ParamStep.Normal, int count = 1)
    {
        var size = kind switch { ParamStep.Fine => Step / 10, ParamStep.Large => Step * LargeSteps, _ => Step };
        var delta = size * count;
        if (Scale == ValueScale.Log) return ValueAt(PositionOf(value) + delta);
        var next = Clamped(value) + delta;
        return kind == ParamStep.Fine ? Clamped(next) : Snapped(next);
    }

    /// <summary>The readout shows the default in <c>muted</c> and a changed value in <c>ink</c>.</summary>
    public bool IsDefault(double value) => Math.Abs(Clamped(value) - DefaultValue) <= Math.Max(1e-9, Step * 1e-6);

    /// <summary>The readout, inside the bounds; a value that is not a number reads «—».</summary>
    public string Text(double value, string decimalSeparator = ",") =>
        double.IsNaN(value) ? NumberText.NotANumber : NumberText.Format(Clamped(value), FractionDigits, Unit, decimalSeparator);
}
