import CoreGraphics
import Foundation

/// Squarified treemap: tiles stay close to square so their labels remain readable.
public enum Treemap {
    /// One rect per value, in the input's order. Zero or negative values get an empty rect.
    public static func layout(_ values: [Double], in bounds: CGRect) -> [CGRect] {
        var result = [CGRect](repeating: .zero, count: values.count)
        let total = values.reduce(0) { $0 + max(0, $1) }
        guard total > 0, bounds.width > 0, bounds.height > 0 else { return result }
        let scale = Double(bounds.width * bounds.height) / total
        var remaining = values.indices.filter { values[$0] > 0 }.sorted { values[$0] > values[$1] }[...]
        var free = bounds
        while !remaining.isEmpty {
            let side = Double(min(free.width, free.height))
            var row: [Int] = [], rowArea = 0.0, best = Double.infinity
            while let next = remaining.first {
                let area = values[next] * scale, candidate = rowArea + area
                let largest = values[row.first ?? next] * scale, smallest = area
                let worst = max(side * side * largest / (candidate * candidate), candidate * candidate / (side * side * smallest))
                if !row.isEmpty && worst > best { break }
                row.append(next); rowArea = candidate; best = worst; remaining.removeFirst()
            }
            let thickness = CGFloat(rowArea / side)
            var offset: CGFloat = 0
            let horizontal = free.width >= free.height
            for index in row {
                let length = CGFloat(values[index] * scale / Double(thickness))
                result[index] = horizontal
                    ? CGRect(x: free.minX, y: free.minY + offset, width: thickness, height: length)
                    : CGRect(x: free.minX + offset, y: free.minY, width: length, height: thickness)
                offset += length
            }
            free = horizontal
                ? CGRect(x: free.minX + thickness, y: free.minY, width: max(0, free.width - thickness), height: free.height)
                : CGRect(x: free.minX, y: free.minY + thickness, width: free.width, height: max(0, free.height - thickness))
        }
        return result
    }
}
