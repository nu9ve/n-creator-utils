namespace Nu9ve.Shell;

/// <summary>Layout rules shared by every Windows app of the suite.</summary>
public static class Layout
{
    /// <summary>Page gutter by width (windows-app-v1 §2, the Windows Settings pattern; origin nove MainWindow.GutterFor).</summary>
    public static double Gutter(double width) => width < 640 ? 16 : width < 1008 ? 24 : 32;

    /// <summary>Clamp a remembered frame inside the work area of an available monitor (windows-app-v1 §2).</summary>
    public static (int X, int Y, int Width, int Height) Fit((int X, int Y, int Width, int Height) frame, (int X, int Y, int Width, int Height) workArea)
    {
        var width = Math.Min(frame.Width, workArea.Width);
        var height = Math.Min(frame.Height, workArea.Height);
        var x = Math.Clamp(frame.X, workArea.X, workArea.X + workArea.Width - width);
        var y = Math.Clamp(frame.Y, workArea.Y, workArea.Y + workArea.Height - height);
        return (x, y, width, height);
    }
}

/// <summary>Squarified treemap. Origin: MediaMenuCore Treemap.swift, same order and rule.</summary>
public static class Treemap
{
    public readonly record struct Rect(double X, double Y, double Width, double Height);

    public static Rect[] Layout(IReadOnlyList<double> values, Rect bounds)
    {
        var result = new Rect[values.Count];
        var total = values.Sum(v => Math.Max(0, v));
        if (total <= 0 || bounds.Width <= 0 || bounds.Height <= 0) return result;
        var scale = bounds.Width * bounds.Height / total;
        var remaining = new Queue<int>(Enumerable.Range(0, values.Count).Where(i => values[i] > 0).OrderByDescending(i => values[i]));
        var free = bounds;
        while (remaining.Count > 0)
        {
            var side = Math.Min(free.Width, free.Height);
            var row = new List<int>();
            double rowArea = 0, best = double.PositiveInfinity;
            while (remaining.TryPeek(out var next))
            {
                double area = values[next] * scale, candidate = rowArea + area;
                double largest = values[row.Count > 0 ? row[0] : next] * scale, smallest = area;
                var worst = Math.Max(side * side * largest / (candidate * candidate), candidate * candidate / (side * side * smallest));
                if (row.Count > 0 && worst > best) break;
                row.Add(next); rowArea = candidate; best = worst; remaining.Dequeue();
            }
            var thickness = rowArea / side;
            double offset = 0;
            var horizontal = free.Width >= free.Height;
            foreach (var index in row)
            {
                var length = values[index] * scale / thickness;
                result[index] = horizontal
                    ? new Rect(free.X, free.Y + offset, thickness, length)
                    : new Rect(free.X + offset, free.Y, length, thickness);
                offset += length;
            }
            free = horizontal
                ? new Rect(free.X + thickness, free.Y, Math.Max(0, free.Width - thickness), free.Height)
                : new Rect(free.X, free.Y + thickness, free.Width, Math.Max(0, free.Height - thickness));
        }
        return result;
    }
}
