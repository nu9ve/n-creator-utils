#if os(macOS)
import Nu9veTheme
import Nu9veAction
import NuMacShell
import SwiftUI

/// The file in hand travelling from one place to the other, or through the reader when verifying.
/// Drawn by the skin: documents between disks in Glass, marching ants in Macintosh, papers with a bevel in
/// Windows 9x and `[====>····]` with a prompt in Terminal. Static under Reduce Motion, decorative for VoiceOver.
///
/// Measured in its first app (2026-11-07): a 30 fps timeline that rebuilt the whole travel kept the main thread
/// ~43 % busy. Here the places and the name are plain views that change only with the record, and only the
/// moving dashes and documents are drawn, at 12 fps, by a Canvas that renders off the main thread. It also
/// stops while the app is inactive.
public struct ActionTravel: View {
    @Environment(\.actionProgressStyle) private var style
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.controlActiveState) private var activeState
    let record: ActionRecord?
    public static let framesPerSecond = 12.0

    public init(record: ActionRecord?) {
        self.record = record
    }

    public var body: some View {
        Group {
            if style.skin == .terminal {
                TimelineView(.animation(minimumInterval: 1 / Self.framesPerSecond, paused: paused)) { timeline in
                    terminal(paused ? 0.35 : timeline.date.timeIntervalSinceReferenceDate)
                }
            } else {
                drawn
            }
        }
        .accessibilityHidden(true)
        .allowsHitTesting(false)
    }

    private var paused: Bool { reduceMotion || activeState == .inactive }
    private var current: ActionCurrent? { record?.current }
    private var releasing: Bool { record?.commonStage == .releasing || current?.action == "release" }
    private var verifying: Bool {
        [.verifying, .reviewing, .scanning].contains(record?.commonStage) || current?.action == "verify"
    }
    private var name: String { current.map(\.name).flatMap { $0.isEmpty ? nil : $0 } ?? "…" }
    private var from: String { current?.from ?? "" }
    private var to: String { current?.to ?? "" }

    private var drawn: some View {
        HStack(spacing: 0) {
            endpoint(icon: verifying ? "externaldrive" : "externaldrive.fill", label: from)
            ZStack {
                TimelineView(.animation(minimumInterval: 1 / Self.framesPerSecond, paused: paused)) { timeline in
                    TravelCanvas(style: style, releasing: releasing,
                                 time: paused ? 0.35 : timeline.date.timeIntervalSinceReferenceDate)
                }
                .frame(height: 30)
                .offset(y: -8)
                // The file in hand is named once, above the way, with its own small bar.
                VStack(spacing: 3) {
                    Text(verbatim: name).font(.caption.weight(.medium)).lineLimit(1).truncationMode(.middle)
                        .foregroundStyle(style.ink)
                    if let file = current?.fraction { FileMeter(fraction: file) }
                }
                .padding(.horizontal, 12)
                .offset(y: current?.fraction == nil ? -38 : -44)
            }
            if verifying {
                endpoint(icon: "checkmark.shield", label: String(localized: "huellas"))
            } else {
                endpoint(icon: releasing ? "checkmark.seal" : "externaldrive.fill.badge.checkmark",
                         label: releasing ? String(localized: "liberado") : to)
            }
        }
        .frame(maxHeight: .infinity)
    }

    private func endpoint(icon: String, label: String) -> some View {
        VStack(spacing: 4) {
            Image(systemName: icon).font(.system(size: 30, weight: .regular)).foregroundStyle(style.ink)
            Text(verbatim: label).font(.caption2.weight(.medium)).foregroundStyle(style.muted).lineLimit(1)
        }
        .frame(width: 120)
    }

    /// Terminal: the travel as a line the prompt redraws. Each line is centered on its own: the travel and the
    /// prompt differ in width, and aligned to each other's leading edge the block read off-centre.
    private func terminal(_ time: Double) -> some View {
        let width = 28
        let position = Int((time * 14).truncatingRemainder(dividingBy: Double(width)))
        let track = (0..<width).map { $0 == position ? ">" : $0 < position ? "=" : "·" }.joined()
        let left = from.isEmpty ? "src" : from
        let right = verifying ? "sha256" : releasing ? "free" : (to.isEmpty ? "dst" : to)
        let command = verifying ? "verify" : releasing ? "release" : "cp --verify"
        return VStack(alignment: .center, spacing: 6) {
            Text(verbatim: "\(left) [\(track)] \(right)")
            Text(verbatim: "$ \(command) \(name)" + (current?.fraction.map { "  \(Int($0 * 100))%" } ?? ""))
                .foregroundStyle(style.muted)
        }
        .font(.system(size: 13, weight: .medium, design: .monospaced))
        .foregroundStyle(style.ink)
        .lineLimit(1)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// The file in hand, small and quiet next to the one overall bar. Decorative like the travel; the bar carries the value.
private struct FileMeter: View {
    @Environment(\.actionProgressStyle) private var style
    let fraction: Double
    var body: some View {
        HStack(spacing: 6) {
            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    Rectangle().fill(style.line.opacity(0.6))
                    Rectangle().fill(style.accentText).frame(width: geometry.size.width * fraction)
                }
            }
            .frame(width: 120, height: 3)
            .clipShape(style.controlShape)
            Text(verbatim: "\(Int(fraction * 100)) %").font(.caption2.monospacedDigit()).foregroundStyle(style.muted)
        }
    }
}

/// Only what moves: the way between the places and three small documents on it, in each skin's era.
private struct TravelCanvas: View {
    let style: ActionProgressStyle
    let releasing: Bool
    let time: Double

    var body: some View {
        Canvas(rendersAsynchronously: true) { context, size in
            let mid = size.height / 2, inset: CGFloat = 8
            var way = Path()
            way.move(to: CGPoint(x: inset, y: mid))
            way.addLine(to: CGPoint(x: size.width - inset, y: mid))
            switch style.skin {
            case .classicMac:
                // Marching ants, the 1-bit selection.
                context.stroke(way, with: .color(style.ink), style: StrokeStyle(lineWidth: 1, dash: [3, 3], dashPhase: CGFloat(-time * 12)))
            case .win9x:
                context.stroke(way, with: .color(Win9xBevel.lightShadow), style: StrokeStyle(lineWidth: 2, dash: [1, 3]))
            case .gothic:
                context.stroke(way, with: .color(style.accent.opacity(0.5)), lineWidth: 1)
            default:
                context.stroke(way, with: .color(style.line),
                               style: StrokeStyle(lineWidth: 1.5, lineCap: .round, dash: [6, 5], dashPhase: CGFloat(-time * 20)))
            }
            let span = size.width - 2 * inset - 26
            for index in 0..<3 {
                let phase = (time * 0.45 + Double(index) / 3).truncatingRemainder(dividingBy: 1)
                let edge = min(1, min(phase, 1 - phase) * 6)
                let center = CGPoint(x: inset + 13 + span * phase, y: mid - 5 * sin(phase * .pi))
                var chip = context
                // Releasing fades a file as it goes; copying keeps it whole to the other place.
                chip.opacity = edge * (releasing ? 1 - phase * 0.7 : 1)
                document(&chip, at: center)
            }
        } symbols: {
            Image(systemName: "doc.fill").font(.system(size: 10)).tag(0)
        }
    }

    private func document(_ context: inout GraphicsContext, at center: CGPoint) {
        let rect = CGRect(x: center.x - 13, y: center.y - 9, width: 26, height: 18)
        var ink = style.ink
        switch style.skin {
        case .classicMac:
            context.fill(Path(rect), with: .color(.white))
            context.stroke(Path(rect), with: .color(style.ink), lineWidth: 1)
        case .win9x:
            // The paper that flew between folders, with its bevel.
            context.fill(Path(rect), with: .color(.white))
            context.stroke(Path { path in
                path.move(to: CGPoint(x: rect.minX, y: rect.maxY))
                path.addLine(to: rect.origin)
                path.addLine(to: CGPoint(x: rect.maxX, y: rect.minY))
            }, with: .color(Win9xBevel.highlight), lineWidth: 1)
            context.stroke(Path { path in
                path.move(to: CGPoint(x: rect.maxX, y: rect.minY))
                path.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY))
                path.addLine(to: CGPoint(x: rect.minX, y: rect.maxY))
            }, with: .color(Win9xBevel.darkShadow), lineWidth: 1)
            ink = .black
        case .gothic:
            context.fill(Path(rect), with: .color(style.raised))
            context.stroke(Path(rect), with: .color(style.accent.opacity(0.6)), lineWidth: 1)
        default:
            context.fill(Path(roundedRect: rect, cornerRadius: 9), with: .color(style.accent))
            ink = style.onAccent
        }
        if let icon = context.resolveSymbol(id: 0) {
            var tinted = context
            tinted.addFilter(.colorMultiply(ink))
            tinted.draw(icon, at: center)
        }
    }
}
#endif
