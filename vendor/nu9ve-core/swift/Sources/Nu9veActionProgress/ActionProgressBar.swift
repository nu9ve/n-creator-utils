#if os(macOS)
import Nu9veTheme
import NuMacShell
import SwiftUI

/// The one bar of a long action, drawn by the skin (nu9ve-interaction-v1 §7). A nil fraction is an
/// indeterminate bar, still visible. It carries the accessible value; the travel above it is decorative.
public struct ActionProgressBar: View {
    @Environment(\.actionProgressStyle) private var style
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.controlActiveState) private var activeState
    let fraction: Double?
    let height: CGFloat?

    /// `height` nil: the skin's own (18 pt in Windows 9x, 10 in the others).
    public init(fraction: Double?, height: CGFloat? = nil) {
        self.fraction = fraction.map { min(1, max(0, $0)) }
        self.height = height
    }

    public var body: some View {
        Group {
            if style.skin == .terminal {
                // A small bar (the indicator, a compact row) draws fewer cells at a smaller size.
                let small = (height ?? 10) < 8
                let cells = small ? 6 : 32, filled = Int((fraction ?? 0) * Double(cells))
                Text(verbatim: "[" + String(repeating: "█", count: filled) + String(repeating: "░", count: cells - filled) + "]")
                    .font(.system(size: small ? 9 : 12, design: .monospaced)).foregroundStyle(style.accentText)
                    .lineLimit(1).minimumScaleFactor(0.5)
            } else {
                GeometryReader { geometry in
                    TimelineView(.animation(minimumInterval: 1 / ActionTravel.framesPerSecond,
                                            paused: fraction != nil || reduceMotion || activeState == .inactive)) { timeline in
                        let width = geometry.size.width
                        let shown = fraction.map { CGFloat($0) * width }
                        let sweep = reduceMotion ? 0.36 : CGFloat((timeline.date.timeIntervalSinceReferenceDate * 0.6).truncatingRemainder(dividingBy: 1))
                        ZStack(alignment: .leading) {
                            track
                            fill.frame(width: shown ?? width * 0.28).offset(x: shown == nil ? (width * 1.28) * sweep - width * 0.28 : 0)
                        }
                        .clipShape(style.controlShape)
                    }
                }
                .frame(height: height ?? (style.skin == .win9x ? 18 : 10))
            }
        }
        .accessibilityElement()
        .accessibilityLabel(Text("Progreso"))
        .accessibilityValue(fraction.map { Text(verbatim: "\(Int($0 * 100)) %") } ?? Text("En curso"))
    }

    @ViewBuilder private var track: some View {
        switch style.skin {
        case .win9x: Color.white.overlay(Win9xBevel(isRaised: false))
        case .classicMac: Color.white.overlay(Rectangle().strokeBorder(style.ink, lineWidth: 1))
        default: style.raised.overlay(style.controlShape.stroke(style.line, lineWidth: 1))
        }
    }

    @ViewBuilder private var fill: some View {
        switch style.skin {
        case .win9x where (height ?? 18) < 12:
            // Too thin for blocks: a solid fill inside the sunken bevel.
            style.accentText.padding(1)
        case .win9x:
            // Separate blocks, as the copy dialog drew them.
            GeometryReader { geometry in
                HStack(spacing: 2) {
                    ForEach(0..<max(1, Int(geometry.size.width / 10)), id: \.self) { _ in Rectangle().fill(style.accentText).frame(width: 8) }
                }.padding(3)
            }
        case .classicMac:
            style.ink.padding(2)
        case .gothic:
            LinearGradient(colors: [style.accent.opacity(0.6), style.accent], startPoint: .leading, endPoint: .trailing)
        default:
            style.accent
        }
    }
}
#endif
