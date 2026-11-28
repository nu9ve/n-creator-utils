import CoreGraphics


public enum WindowMode: String, Sendable {
    case compact, selection, full
    public var width: Double { switch self { case .compact: 440; case .selection: 780; case .full: 1180 } }
    public var height: Double { switch self { case .compact: 700; case .selection: 820; case .full: 900 } }
}

/// Where a window goes when it has no saved frame (U1.5, 2026-11-27).
public enum WindowPlacement {
    /// The compact window: its center at three quarters of the screen's width and a little above its middle, so it
    /// floats beside what the person is doing instead of over it. Always inside `visible`.
    public static func compact(width: Double, height: Double, in visible: CGRect) -> CGRect {
        let width = min(width, visible.width), height = min(height, visible.height)
        let x = visible.minX + visible.width * 0.75 - width / 2
        let y = visible.minY + visible.height * 0.56 - height / 2
        return CGRect(x: min(max(visible.minX, x), visible.maxX - width),
                      y: min(max(visible.minY, y), visible.maxY - height), width: width, height: height)
    }
}
