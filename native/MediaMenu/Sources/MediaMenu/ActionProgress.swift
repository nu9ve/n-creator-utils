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

    /// A closeout apply: one overall measure from the engine (`overall_done`/`overall_total`).
    static func transfer(_ value: JSONValue, phase: String) -> Self {
        let total = value["overall_total"].double, done = value["overall_done"].double
        let current = value["current"]
        let log = value["log"].array.enumerated().map { index, line in
            LogLine(id: index, kind: line["kind"].string, name: line["name"].string, target: line["target"].string)
        }
        return Self(kind: .transfer, phase: phase, fraction: total > 0 ? min(1, done / total) : nil, bytesDone: done, bytesTotal: total,
                    filesDone: Int(value["files_done"].double), filesTotal: Int(value["files_total"].double),
                    fileName: current["name"].string, from: current["from"].string, to: current["to"].string,
                    action: current["action"].string, log: log)
    }

    /// A review: reading files to verify them, per step and per file.
    static func verify(_ value: JSONValue, phase: String, volume: String) -> Self {
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
    }

    private var summary: String {
        var parts: [String] = []
        if let fraction = state.fraction { parts.append("\(Int((fraction * 100).rounded(.down))) %") }
        if state.bytesTotal > 0 { parts.append(String(localized: "\(bytesLabel(state.bytesDone)) de \(bytesLabel(state.bytesTotal))")) }
        if state.filesTotal > 0 { parts.append(String(localized: "\(state.filesDone) de \(state.filesTotal) archivos")) }
        return parts.joined(separator: " · ")
    }

    private var details: some View {
        VStack(alignment: .leading, spacing: 8) {
            if !state.fileName.isEmpty {
                HStack(spacing: 6) {
                    Image(systemName: icon(state.action)).foregroundStyle(color(state.action))
                    Text(verbatim: state.fileName).lineLimit(1).truncationMode(.middle)
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
                                    if !line.target.isEmpty { Text(verbatim: (line.kind == "released" ? "· " : "→ ") + line.target).foregroundStyle(palette.muted) }
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
        switch kind { case "copy", "copied": "doc.on.doc"; case "release", "released": "arrow.right.circle"; default: "doc.text.magnifyingglass" }
    }
    private func color(_ kind: String) -> Color {
        switch kind { case "copy", "copied": palette.info; case "release", "released": palette.warning; default: palette.muted }
    }
    private func mark(_ kind: String) -> String { kind == "released" ? "−" : "✓" }
    private func verb(_ kind: String) -> String {
        switch kind {
        case "copied": String(localized: "copiado y verificado")
        case "released": String(localized: "retirado de edición")
        default: kind
        }
    }
}

/// The file in hand travelling from one disk to the other (or through the reader, when verifying).
/// Drawn by the skin; static under Reduce Motion.
struct ActionTravel: View {
    @Environment(\.creatorPalette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let state: ActionProgressState

    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / 30, paused: reduceMotion)) { timeline in
            let time = reduceMotion ? 0.35 : timeline.date.timeIntervalSinceReferenceDate
            if palette.skin == .terminal { terminal(time) } else { drawn(time) }
        }
        .accessibilityHidden(true)
    }

    private var releasing: Bool { state.action == "release" }
    private var verifying: Bool { state.kind == .verify }
    private var name: String { state.fileName.isEmpty ? "…" : state.fileName }

    private func drawn(_ time: Double) -> some View {
        GeometryReader { geometry in
            let width = geometry.size.width, mid = geometry.size.height / 2 - 8
            let left = CGPoint(x: 44, y: mid), right = CGPoint(x: width - 44, y: mid)
            ZStack {
                path(from: left, to: right, time: time)
                // The file in hand is named once, above the way; small documents travel along it.
                Text(verbatim: name).font(.caption.weight(.medium)).lineLimit(1).truncationMode(.middle)
                    .foregroundStyle(palette.ink).frame(maxWidth: max(80, right.x - left.x - 110))
                    .position(x: width / 2, y: mid - 30)
                ForEach(0..<3) { index in
                    let phase = (time * 0.45 + Double(index) / 3).truncatingRemainder(dividingBy: 1)
                    chip("", phase: phase)
                        .position(x: left.x + 46 + (right.x - left.x - 92) * phase, y: mid - 5 * sin(phase * .pi))
                        .opacity(fade(phase, releasing: releasing))
                }
                endpoint(icon: verifying ? "externaldrive" : "externaldrive.fill", label: state.from).position(left)
                if !verifying {
                    endpoint(icon: releasing ? "checkmark.seal" : "externaldrive.fill.badge.checkmark",
                             label: releasing ? String(localized: "liberado") : state.to).position(right)
                } else {
                    endpoint(icon: "checkmark.shield", label: String(localized: "huellas")).position(right)
                }
            }
        }
    }

    /// Releasing fades a file as it goes; copying keeps it whole to the other disk.
    private func fade(_ phase: Double, releasing: Bool) -> Double {
        let edge = min(phase, 1 - phase) * 6
        return min(1, edge) * (releasing ? 1 - phase * 0.7 : 1)
    }

    @ViewBuilder private func path(from: CGPoint, to: CGPoint, time: Double) -> some View {
        let line = Path { $0.move(to: CGPoint(x: from.x + 30, y: from.y)); $0.addLine(to: CGPoint(x: to.x - 30, y: to.y)) }
        switch palette.skin {
        case .classicMac:
            // Marching ants, the 1-bit selection.
            line.stroke(palette.ink, style: StrokeStyle(lineWidth: 1, dash: [3, 3], dashPhase: CGFloat(-time * 12)))
        case .win9x:
            line.stroke(Win9xBevel.lightShadow, style: StrokeStyle(lineWidth: 2, dash: [1, 3]))
        case .gothic:
            line.stroke(palette.accent.opacity(0.5), style: StrokeStyle(lineWidth: 1)).shadow(color: palette.accent.opacity(0.6), radius: 4)
        default:
            line.stroke(palette.line, style: StrokeStyle(lineWidth: 1.5, lineCap: .round, dash: [6, 5], dashPhase: CGFloat(-time * 20)))
        }
    }

    @ViewBuilder private func chip(_ text: String, phase: Double) -> some View {
        let label = HStack(spacing: 4) {
            Image(systemName: "doc.fill").font(.system(size: 10))
            if !text.isEmpty { Text(verbatim: text).font(.system(size: 10, weight: .medium)).lineLimit(1).truncationMode(.middle) }
        }
        .padding(.horizontal, 6).padding(.vertical, 3)
        .frame(maxWidth: text.isEmpty ? 26 : 180)
        switch palette.skin {
        case .classicMac:
            label.foregroundStyle(palette.ink).background(Color.white).overlay(Rectangle().stroke(palette.ink, lineWidth: 1))
        case .win9x:
            // The paper that flew between folders.
            label.foregroundStyle(Color.black).background(Color.white).overlay(Win9xBevel())
        case .gothic:
            label.foregroundStyle(palette.ink).background(palette.raised, in: Rectangle())
                .overlay(Rectangle().stroke(palette.accent.opacity(0.6), lineWidth: 1)).shadow(color: palette.accent.opacity(0.4), radius: 3)
        default:
            label.foregroundStyle(palette.onAccent).background(palette.accent, in: palette.pill)
        }
    }

    private func endpoint(icon: String, label: String) -> some View {
        VStack(spacing: 4) {
            Image(systemName: icon).font(.system(size: 30, weight: .regular)).foregroundStyle(palette.ink)
            Text(verbatim: label).font(.caption2.weight(.medium)).foregroundStyle(palette.muted).lineLimit(1)
        }
        .frame(width: 88)
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
            Text(verbatim: "$ \(verifying ? "verify" : releasing ? "release" : "cp --verify") \(name)").foregroundStyle(palette.muted)
        }
        .font(.system(size: 13, weight: .medium, design: .monospaced))
        .foregroundStyle(palette.ink)
        .lineLimit(1)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// The one bar of a long action, drawn by the skin. Nil fraction: working, amount unknown.
struct SkinProgressBar: View {
    @Environment(\.creatorPalette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let fraction: Double?

    var body: some View {
        Group {
            if palette.skin == .terminal {
                let cells = 32, filled = Int((fraction ?? 0) * Double(cells))
                Text(verbatim: "[" + String(repeating: "█", count: filled) + String(repeating: "░", count: cells - filled) + "]")
                    .font(.system(size: 12, design: .monospaced)).foregroundStyle(palette.accentText)
            } else {
                GeometryReader { geometry in
                    TimelineView(.animation(minimumInterval: 1 / 30, paused: fraction != nil || reduceMotion)) { timeline in
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
