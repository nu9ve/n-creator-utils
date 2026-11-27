import CoreGraphics

/// Where blocks sit in a pipeline stage, so the lines between stages need no measuring.
public enum FlowGeometry {
    /// Vertical centers of `count` blocks, centered as a group in `height`.
    public static func centers(count: Int, blockHeight: CGFloat, spacing: CGFloat, height: CGFloat) -> [CGFloat] {
        guard count > 0 else { return [] }
        let used = CGFloat(count) * blockHeight + CGFloat(count - 1) * spacing
        let top = (height - used) / 2
        return (0..<count).map { top + blockHeight / 2 + CGFloat($0) * (blockHeight + spacing) }
    }
    /// Blocks share the width after the connectors, within limits; past the upper one the row stays left-aligned.
    public static func blockWidth(available: CGFloat, stages: Int, connector: CGFloat, range: ClosedRange<CGFloat>) -> CGFloat {
        guard stages > 0 else { return range.lowerBound }
        let room = (available - connector * CGFloat(stages - 1)) / CGFloat(stages)
        return min(max(room, range.lowerBound), range.upperBound)
    }
}
