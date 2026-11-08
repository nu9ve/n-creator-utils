import SwiftUI
import MediaMenuCore

// nu9ve-interaction-v1 §7 · «Acciones largas»: one view for any long action (copy, verify, release, ingest).
// While the action runs it replaces the screen it started from: what is happening, one bar that is always
// visible, the file in hand travelling between disks, and a short log behind a disclosure. Each skin draws
// it in its own era; Reduce Motion stops the travel and keeps every number.

/// What the engine reported, already read from JSON, so the view never parses.
struct ActionProgressState: Equatable {
    enum Kind { case transfer, verify }
    struct LogLine: Equatable, Identifiable {
        let id: Int
        let kind: String
        let name: String
        let target: String
    }
    var kind: Kind
    var phase: String
    var fraction: Double?
    var bytesDone: Double
    var bytesTotal: Double
    var filesDone: Int
    var filesTotal: Int
    var fileName: String
    var from: String
    var to: String
    var action: String
    var log: [LogLine]
    /// The file in hand (`file_done`/`file_total`), for the small indicator beside the travel. Nil when unknown.
    var fileFraction: Double? = nil

    private static func file(_ value: JSONValue) -> Double? {
        let total = value["file_total"].double
        return total > 0 ? min(1, value["file_done"].double / total) : nil
    }

    /// Any long action with one overall measure from the engine (`overall_done`/`overall_total`): closeout, ingest,
    /// release or delivery from the archive (media/progress.py).
    static func transfer(_ value: JSONValue, phase: String) -> Self {
        let total = value["overall_total"].double, done = value["overall_done"].double
        let current = value["current"]
        let log = value["log"].array.enumerated().map { index, line in
            LogLine(id: index, kind: line["kind"].string, name: line["name"].string, target: line["target"].string)
        }
        return Self(kind: .transfer, phase: phase, fraction: total > 0 ? min(1, done / total) : nil, bytesDone: done, bytesTotal: total,
                    filesDone: Int(value["files_done"].double), filesTotal: Int(value["files_total"].double),
                    fileName: current["name"].string, from: current["from"].string, to: current["to"].string,
                    action: current["action"].string, log: log, fileFraction: file(value))
    }

    /// A review: reading files to verify them. The engine gives one measure for the whole review (closure.ReviewMeter);
    /// older reports without it fall back to the current step.
    static func verify(_ value: JSONValue, phase: String, volume: String) -> Self {
        if value["overall_total"].double > 0 {
            let total = value["overall_total"].double, current = value["current"]
            return Self(kind: .verify, phase: phase, fraction: min(1, value["overall_done"].double / total),
                        bytesDone: value["overall_done"].double, bytesTotal: total,
                        filesDone: Int(value["overall_files_done"].double), filesTotal: Int(value["overall_files_total"].double),
                        fileName: current["name"].string, from: current["from"].string.isEmpty ? volume : current["from"].string,
                        to: "", action: "verify", log: [], fileFraction: file(value))
        }
        let files = value["files_total"].double, size = value["bytes_total"].double
        let fraction: Double? = files > 0 ? min(1, (value["files_done"].double + (size > 0 ? value["bytes_done"].double / size : 0)) / files)
            : size > 0 ? min(1, value["bytes_done"].double / size) : nil
        let path = value["path"].string
        return Self(kind: .verify, phase: phase, fraction: fraction, bytesDone: value["bytes_done"].double, bytesTotal: size,
                    filesDone: Int(value["files_done"].double), filesTotal: Int(files),
                    fileName: path.isEmpty ? "" : (path as NSString).lastPathComponent, from: volume, to: "", action: "verify", log: [])
    }
}

struct ActionProgressView: View {
    @Environment(\.creatorPalette) private var palette
    let title: LocalizedStringKey
    let note: LocalizedStringKey
    let state: ActionProgressState
    @State private var showDetails = false
    /// First measured point of this action, to estimate what is left from its real pace.
    @State private var origin: (date: Date, bytes: Double)?
    @State private var remaining: TimeInterval?

    var body: some View {
        VStack(spacing: 18) {
            Spacer(minLength: 0)
            VStack(spacing: 6) {
                Text(title).font(palette.heading(.title3))
                Text(verbatim: state.phase).font(.callout).foregroundStyle(palette.muted)
            }
            ActionTravel(state: state).frame(height: 120).frame(maxWidth: 560)
            VStack(spacing: 8) {
                SkinProgressBar(fraction: state.fraction).frame(maxWidth: 560)
                Text(verbatim: summary).font(.caption.monospacedDigit()).foregroundStyle(palette.muted)
            }
            Text(note).font(.caption).foregroundStyle(palette.muted).multilineTextAlignment(.center).frame(maxWidth: 520)
            DisclosureGroup(isExpanded: $showDetails) { details } label: {
                Text("Detalle").font(.callout)
            }
            .frame(maxWidth: 560)
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityElement(children: .contain)
        .onChange(of: state.bytesDone) { _, done in estimate(done) }
    }

    /// What is left, from the pace of the one overall measure since it started; shown after a few seconds of data.
    private func estimate(_ done: Double) {
        guard state.bytesTotal > 0, state.fraction != nil else { remaining = nil; return }
        guard let start = origin, done >= start.bytes else { origin = (Date(), done); remaining = nil; return }
        let elapsed = Date().timeIntervalSince(start.date), moved = done - start.bytes
        remaining = elapsed >= 5 && moved > 0 ? (state.bytesTotal - done) * elapsed / moved : nil
    }

    private var summary: String {
        var parts: [String] = []
        if let fraction = state.fraction { parts.append("\(Int((fraction * 100).rounded(.down))) %") }
        if state.bytesTotal > 0 { parts.append(String(localized: "\(bytesLabel(state.bytesDone)) de \(bytesLabel(state.bytesTotal))")) }
        if state.filesTotal > 0 { parts.append(String(localized: "\(state.filesDone) de \(state.filesTotal) archivos")) }
        if let remaining, remaining > 0 {
            let minutes = Int((remaining / 60).rounded(.up))
            parts.append(minutes <= 1 ? String(localized: "queda menos de un minuto") : String(localized: "quedan ~\(minutes) min"))
        }
        return parts.joined(separator: " · ")
    }

    private var details: some View {
        VStack(alignment: .leading, spacing: 8) {
            if !state.fileName.isEmpty {
                HStack(spacing: 6) {
                    Image(systemName: icon(state.action)).foregroundStyle(color(state.action))
                    Text(verbatim: state.fileName).lineLimit(1).truncationMode(.middle)
                    if let file = state.fileFraction { Text(verbatim: "\(Int(file * 100)) %").monospacedDigit().foregroundStyle(palette.muted) }
                    Spacer()
                    Text(verbatim: route(state.from, state.to)).foregroundStyle(palette.muted)
                }.font(.caption)
            }
            if state.log.isEmpty {
                Text("Todavía no hay pasos terminados.").font(.caption).foregroundStyle(palette.muted)
            } else {
                ScrollViewReader { proxy in
                    ScrollView {
                        VStack(alignment: .leading, spacing: 3) {
                            ForEach(state.log) { line in
                                HStack(spacing: 6) {
                                    Text(verbatim: mark(line.kind)).foregroundStyle(color(line.kind))
                                    Text(verbatim: verb(line.kind)).foregroundStyle(palette.muted)
                                    Text(verbatim: line.name).lineLimit(1).truncationMode(.middle)
                                    if !line.target.isEmpty { Text(verbatim: (["released", "cleaned"].contains(line.kind) ? "· " : "→ ") + line.target).foregroundStyle(palette.muted) }
                                }
                                .id(line.id)
                            }
                        }
                        .font(.system(size: 11, design: .monospaced))
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(8)
                    }
                    .frame(maxHeight: 150)
                    .background(palette.raised, in: palette.control)
                    .onChange(of: state.log.count) { _, _ in if let last = state.log.last { proxy.scrollTo(last.id, anchor: .bottom) } }
                }
            }
        }
        .padding(.top, 6)
    }

    private func route(_ from: String, _ to: String) -> String { to.isEmpty ? from : from.isEmpty ? to : from + " → " + to }
    private func icon(_ kind: String) -> String {
        switch kind { case "copy", "copied": "doc.on.doc"; case "release", "released", "cleaned": "arrow.right.circle"; default: "doc.text.magnifyingglass" }
    }
    private func color(_ kind: String) -> Color {
        switch kind { case "copy", "copied": palette.info; case "release", "released", "cleaned": palette.warning; default: palette.muted }
    }
    private func mark(_ kind: String) -> String { ["released", "cleaned"].contains(kind) ? "−" : "✓" }
    private func verb(_ kind: String) -> String {
        switch kind {
        case "copied": String(localized: "copiado y verificado")
        case "released": String(localized: "retirado de edición")
        case "cleaned": String(localized: "retirado del origen")
        default: kind
        }
    }
}

/// The file in hand travelling from one disk to the other (or through the reader, when verifying).
/// Drawn by the skin; static under Reduce Motion.
///
/// Measured 2026-11-07 (STATUS): a 30 fps TimelineView that rebuilt the whole travel kept the main thread ~43 % busy,
/// mostly waiting on WindowServer for CPU-drawn layers and relayouting the window. Now the disks and the file name are
/// plain views that change only with the state, and only the moving dashes and documents are drawn, at 12 fps, by a
/// Canvas that renders off the main thread. It also stops while the app is inactive.
struct ActionTravel: View {
    @Environment(\.creatorPalette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.controlActiveState) private var activeState
    let state: ActionProgressState
    static let framesPerSecond = 12.0

    var body: some View {
        Group {
            if palette.skin == .terminal {
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
    private var releasing: Bool { state.action == "release" }
    private var verifying: Bool { state.kind == .verify }
    private var name: String { state.fileName.isEmpty ? "…" : state.fileName }

    private var drawn: some View {
        HStack(spacing: 0) {
            endpoint(icon: verifying ? "externaldrive" : "externaldrive.fill", label: state.from)
            ZStack {
                TimelineView(.animation(minimumInterval: 1 / Self.framesPerSecond, paused: paused)) { timeline in
                    TravelCanvas(skin: palette.skin, palette: palette, releasing: releasing,
                                 time: paused ? 0.35 : timeline.date.timeIntervalSinceReferenceDate)
                }
                .frame(height: 30)
                .offset(y: -8)
                // The file in hand is named once, above the way, with its own small bar.
                VStack(spacing: 3) {
                    Text(verbatim: name).font(.caption.weight(.medium)).lineLimit(1).truncationMode(.middle)
                        .foregroundStyle(palette.ink)
                    if let file = state.fileFraction { FileMeter(fraction: file) }
                }
                .padding(.horizontal, 12)
                .offset(y: state.fileFraction == nil ? -38 : -44)
            }
            if !verifying {
                endpoint(icon: releasing ? "checkmark.seal" : "externaldrive.fill.badge.checkmark",
                         label: releasing ? String(localized: "liberado") : state.to)
            } else {
                endpoint(icon: "checkmark.shield", label: String(localized: "huellas"))
            }
        }
        .frame(maxHeight: .infinity)
    }

    private func endpoint(icon: String, label: String) -> some View {
        VStack(spacing: 4) {
            Image(systemName: icon).font(.system(size: 30, weight: .regular)).foregroundStyle(palette.ink)
            Text(verbatim: label).font(.caption2.weight(.medium)).foregroundStyle(palette.muted).lineLimit(1)
        }
        .frame(width: 120)
    }

    /// Terminal: the travel as a line the prompt redraws.
    private func terminal(_ time: Double) -> some View {
        let width = 28
        let position = Int((time * 14).truncatingRemainder(dividingBy: Double(width)))
        let track = (0..<width).map { $0 == position ? ">" : $0 < position ? "=" : "·" }.joined()
        let left = state.from.isEmpty ? "src" : state.from
        let right = verifying ? "sha256" : releasing ? "free" : (state.to.isEmpty ? "dst" : state.to)
        return VStack(alignment: .leading, spacing: 6) {
            Text(verbatim: "\(left) [\(track)] \(right)")
            Text(verbatim: "$ \(verifying ? "verify" : releasing ? "release" : "cp --verify") \(name)" + (state.fileFraction.map { "  \(Int($0 * 100))%" } ?? ""))
                .foregroundStyle(palette.muted)
        }
        .font(.system(size: 13, weight: .medium, design: .monospaced))
        .foregroundStyle(palette.ink)
        .lineLimit(1)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// The file in hand, small and quiet next to the one overall bar. Decorative like the travel; the bar carries the value.
private struct FileMeter: View {
    @Environment(\.creatorPalette) private var palette
    let fraction: Double
    var body: some View {
        HStack(spacing: 6) {
            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    Rectangle().fill(palette.line.opacity(0.6))
                    Rectangle().fill(palette.accentText).frame(width: geometry.size.width * fraction)
                }
            }
            .frame(width: 120, height: 3)
            .clipShape(palette.skin == .glass ? AnyShape(Capsule()) : AnyShape(Rectangle()))
            Text(verbatim: "\(Int(fraction * 100)) %").font(.caption2.monospacedDigit()).foregroundStyle(palette.muted)
        }
    }
}

/// Only what moves: the way between the disks and three small documents on it, in each skin's era.
private struct TravelCanvas: View {
    let skin: ThemeSkin
    let palette: CreatorPalette
    let releasing: Bool
    let time: Double

    var body: some View {
        Canvas(rendersAsynchronously: true) { context, size in
            let mid = size.height / 2, inset: CGFloat = 8
            var way = Path()
            way.move(to: CGPoint(x: inset, y: mid)); way.addLine(to: CGPoint(x: size.width - inset, y: mid))
            switch skin {
            case .classicMac:
                // Marching ants, the 1-bit selection.
                context.stroke(way, with: .color(palette.ink), style: StrokeStyle(lineWidth: 1, dash: [3, 3], dashPhase: CGFloat(-time * 12)))
            case .win9x:
                context.stroke(way, with: .color(Win9xBevel.lightShadow), style: StrokeStyle(lineWidth: 2, dash: [1, 3]))
            case .gothic:
                context.stroke(way, with: .color(palette.accent.opacity(0.5)), lineWidth: 1)
            default:
                context.stroke(way, with: .color(palette.line), style: StrokeStyle(lineWidth: 1.5, lineCap: .round, dash: [6, 5], dashPhase: CGFloat(-time * 20)))
            }
            let span = size.width - 2 * inset - 26
            for index in 0..<3 {
                let phase = (time * 0.45 + Double(index) / 3).truncatingRemainder(dividingBy: 1)
                let edge = min(1, min(phase, 1 - phase) * 6)
                let center = CGPoint(x: inset + 13 + span * phase, y: mid - 5 * sin(phase * .pi))
                var chip = context
                // Releasing fades a file as it goes; copying keeps it whole to the other disk.
                chip.opacity = edge * (releasing ? 1 - phase * 0.7 : 1)
                document(&chip, at: center)
            }
        } symbols: {
            Image(systemName: "doc.fill").font(.system(size: 10)).tag(0)
        }
    }

    private func document(_ context: inout GraphicsContext, at center: CGPoint) {
        let rect = CGRect(x: center.x - 13, y: center.y - 9, width: 26, height: 18)
        var ink = palette.ink
        switch skin {
        case .classicMac:
            context.fill(Path(rect), with: .color(.white))
            context.stroke(Path(rect), with: .color(palette.ink), lineWidth: 1)
        case .win9x:
            // The paper that flew between folders, with its bevel.
            context.fill(Path(rect), with: .color(.white))
            context.stroke(Path { $0.move(to: CGPoint(x: rect.minX, y: rect.maxY)); $0.addLine(to: rect.origin); $0.addLine(to: CGPoint(x: rect.maxX, y: rect.minY)) },
                           with: .color(Win9xBevel.highlight), lineWidth: 1)
            context.stroke(Path { $0.move(to: CGPoint(x: rect.maxX, y: rect.minY)); $0.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY)); $0.addLine(to: CGPoint(x: rect.minX, y: rect.maxY)) },
                           with: .color(Win9xBevel.darkShadow), lineWidth: 1)
            ink = .black
        case .gothic:
            context.fill(Path(rect), with: .color(palette.raised))
            context.stroke(Path(rect), with: .color(palette.accent.opacity(0.6)), lineWidth: 1)
        default:
            context.fill(Path(roundedRect: rect, cornerRadius: 9), with: .color(palette.accent))
            ink = palette.onAccent
        }
        if let icon = context.resolveSymbol(id: 0) {
            var tinted = context
            tinted.addFilter(.colorMultiply(ink))
            tinted.draw(icon, at: center)
        }
    }
}

/// The one bar of a long action, drawn by the skin. Nil fraction: working, amount unknown.
struct SkinProgressBar: View {
    @Environment(\.creatorPalette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.controlActiveState) private var activeState
    let fraction: Double?

    var body: some View {
        Group {
            if palette.skin == .terminal {
                let cells = 32, filled = Int((fraction ?? 0) * Double(cells))
                Text(verbatim: "[" + String(repeating: "█", count: filled) + String(repeating: "░", count: cells - filled) + "]")
                    .font(.system(size: 12, design: .monospaced)).foregroundStyle(palette.accentText)
            } else {
                GeometryReader { geometry in
                    TimelineView(.animation(minimumInterval: 1 / ActionTravel.framesPerSecond, paused: fraction != nil || reduceMotion || activeState == .inactive)) { timeline in
                        let width = geometry.size.width
                        let shown = fraction.map { CGFloat($0) * width }
                        let sweep = CGFloat((timeline.date.timeIntervalSinceReferenceDate * 0.6).truncatingRemainder(dividingBy: 1))
                        ZStack(alignment: .leading) {
                            track
                            fill.frame(width: shown ?? width * 0.28).offset(x: shown == nil ? (width * 1.28) * sweep - width * 0.28 : 0)
                        }
                        .clipShape(shape)
                    }
                }
                .frame(height: palette.skin == .win9x ? 18 : 10)
            }
        }
        .accessibilityElement()
        .accessibilityLabel(Text("Progreso"))
        .accessibilityValue(Text(verbatim: fraction.map { "\(Int($0 * 100)) %" } ?? String(localized: "En curso")))
    }

    private var shape: AnyShape { palette.skin == .glass ? AnyShape(Capsule()) : AnyShape(Rectangle()) }

    @ViewBuilder private var track: some View {
        switch palette.skin {
        case .win9x: Color.white.overlay(Win9xBevel(isRaised: false))
        case .classicMac: Color.white.overlay(Rectangle().stroke(palette.ink, lineWidth: 1))
        default: palette.raised.overlay(shape.stroke(palette.line, lineWidth: 1))
        }
    }

    @ViewBuilder private var fill: some View {
        switch palette.skin {
        case .win9x:
            // Separate blocks, as the copy dialog drew them.
            GeometryReader { geometry in
                HStack(spacing: 2) {
                    ForEach(0..<max(1, Int(geometry.size.width / 10)), id: \.self) { _ in Rectangle().fill(palette.accentText).frame(width: 8) }
                }.padding(3)
            }
        case .classicMac:
            palette.ink.padding(2)
        case .gothic:
            LinearGradient(colors: [palette.accent.opacity(0.6), palette.accent], startPoint: .leading, endPoint: .trailing)
        default:
            palette.accent
        }
    }
}
