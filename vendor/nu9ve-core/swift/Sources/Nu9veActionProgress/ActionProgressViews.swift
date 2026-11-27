#if os(macOS)
import Nu9veTheme
import Nu9veAction
import SwiftUI

// nu9ve-interaction-v1 §7 «Acciones largas»: the same action in three sizes, with the same bar, the same
// travel and the same numbers, and the same closing for every action.

/// Size 1, the screen: it replaces the view the action started from. Running, it says what is done, the file
/// in hand, one bar that is always visible and a short log behind a disclosure. Finished, it gives way to the
/// closing (`ActionClosingView`).
public struct ActionProgressScreen<Actions: View>: View {
    @Environment(\.actionProgressStyle) private var style
    let snapshot: ActionSnapshot
    let title: LocalizedStringKey?
    let note: LocalizedStringKey?
    let actions: Actions
    @State private var showDetails = false

    /// `title`: a concrete verb («Copiando y verificando finales»); nil uses the record's `label`.
    /// `note`: what happens if it is paused or cancelled. `actions`: the app's buttons (Cancelar, Pausar),
    /// shown while it runs and, under the closing, once it ends.
    public init(_ snapshot: ActionSnapshot, title: LocalizedStringKey? = nil, note: LocalizedStringKey? = nil,
                @ViewBuilder actions: () -> Actions) {
        self.snapshot = snapshot
        self.title = title
        self.note = note
        self.actions = actions()
    }

    public var body: some View {
        VStack(spacing: 18) {
            Spacer(minLength: 0)
            if snapshot.display.isFinished {
                ActionClosingView(snapshot)
            } else {
                running
            }
            actions
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityElement(children: .contain)
    }

    @ViewBuilder private var running: some View {
        VStack(spacing: 6) {
            headline.font(.title3.weight(.semibold)).foregroundStyle(style.ink)
            ActionStatusLine(snapshot: snapshot).font(.callout)
        }
        ActionTravel(record: snapshot.record).frame(height: 120).frame(maxWidth: 560)
        VStack(spacing: 8) {
            ActionProgressBar(fraction: snapshot.fraction).frame(maxWidth: 560)
            Text(verbatim: ActionText.summary(snapshot)).font(.caption.monospacedDigit()).foregroundStyle(style.muted)
        }
        if let note {
            Text(note).font(.caption).foregroundStyle(style.muted).multilineTextAlignment(.center).frame(maxWidth: 520)
        }
        DisclosureGroup(isExpanded: $showDetails) { ActionDetails(record: snapshot.record) } label: {
            Text("Detalle").font(.callout).foregroundStyle(style.ink)
        }
        .tint(style.ink)
        .frame(maxWidth: 560)
    }

    private var headline: Text {
        if let title { return Text(title) }
        if let label = snapshot.record?.label { return Text(verbatim: label) }
        return Text("Preparando")
    }
}

extension ActionProgressScreen where Actions == EmptyView {
    public init(_ snapshot: ActionSnapshot, title: LocalizedStringKey? = nil, note: LocalizedStringKey? = nil) {
        self.init(snapshot, title: title, note: note) { EmptyView() }
    }
}

/// The stage, the phase and step, or why the numbers are not moving.
struct ActionStatusLine: View {
    @Environment(\.actionProgressStyle) private var style
    let snapshot: ActionSnapshot

    var body: some View {
        Group {
            if snapshot.display == .noSignal {
                Text("Sin señal de la acción desde hace \(Int(snapshot.silentSeconds ?? 0)) s").foregroundStyle(style.warning)
            } else if let record = snapshot.record {
                HStack(spacing: 6) {
                    Text(ActionText.stage(record))
                    if let phase = record.phase { Text(verbatim: "· \(phase.index)/\(phase.count) \(phase.label)") }
                    if let step = record.step {
                        Text(verbatim: "· \(step.label)" + (step.fraction.map { " \(Int($0 * 100)) %" } ?? ""))
                    }
                }
                .foregroundStyle(style.muted)
            } else {
                Text("Preparando").foregroundStyle(style.muted)
            }
        }
        .lineLimit(1)
        .truncationMode(.middle)
    }
}

/// The file in hand and the short log the engine writes, newest at the bottom.
struct ActionDetails: View {
    @Environment(\.actionProgressStyle) private var style
    let record: ActionRecord?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let current = record?.current, !current.name.isEmpty {
                HStack(spacing: 6) {
                    Circle().fill(style.color(forKind: current.action)).frame(width: 6, height: 6)
                    Text(verbatim: current.name).lineLimit(1).truncationMode(.middle)
                    if let file = current.fraction {
                        Text(verbatim: "\(Int(file * 100)) %").monospacedDigit().foregroundStyle(style.muted)
                    }
                    Spacer()
                    Text(verbatim: ActionText.route(current.from, current.to)).foregroundStyle(style.muted)
                }
                .font(.caption)
            }
            let log = record?.log ?? []
            if log.isEmpty {
                Text("Todavía no hay pasos terminados.").font(.caption).foregroundStyle(style.muted)
            } else {
                ScrollViewReader { proxy in
                    ScrollView {
                        VStack(alignment: .leading, spacing: 3) {
                            ForEach(log) { line in
                                HStack(spacing: 6) {
                                    Text(verbatim: mark(line.kind)).foregroundStyle(style.color(forKind: line.kind))
                                    Text(verbatim: ActionText.verb(line)).foregroundStyle(style.muted)
                                    Text(verbatim: line.name).lineLimit(1).truncationMode(.middle)
                                    if !line.target.isEmpty {
                                        Text(verbatim: (removes(line.kind) ? "· " : "→ ") + line.target).foregroundStyle(style.muted)
                                    }
                                }
                                .id(line.id)
                            }
                        }
                        .font(.system(size: 11, design: .monospaced))
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(8)
                    }
                    .frame(maxHeight: 150)
                    .background(style.raised, in: RoundedRectangle(cornerRadius: CGFloat(style.skin.controlRadius)))
                    .onChange(of: log.count) { _, _ in if let last = log.last { proxy.scrollTo(last.id, anchor: .bottom) } }
                }
            }
        }
        .padding(.top, 6)
    }

    private func removes(_ kind: String) -> Bool { ["released", "cleaned", "removed"].contains(kind) }
    private func mark(_ kind: String) -> String { removes(kind) ? "−" : kind == "failed" ? "×" : "✓" }
}

/// The closing, the same for every action: success, done with a warning, error or cancelled. Under Reduce
/// Motion it keeps every text and drops the animation. The app adds Reanudar, Reintentar or Ver el recibo as
/// actions of the screen; `resumable` in the snapshot says which.
public struct ActionClosingView: View {
    @Environment(\.actionProgressStyle) private var style
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let snapshot: ActionSnapshot
    @State private var appeared = false

    public init(_ snapshot: ActionSnapshot) {
        self.snapshot = snapshot
    }

    public var body: some View {
        VStack(spacing: 10) {
            mark
                .font(.system(size: 34, weight: .regular))
                .foregroundStyle(tint)
                .scaleEffect(appeared || reduceMotion ? 1 : 0.6)
                .opacity(appeared || reduceMotion ? 1 : 0)
                .accessibilityHidden(true)
            headline.font(.title3.weight(.semibold)).foregroundStyle(style.ink).multilineTextAlignment(.center)
            if let summary = snapshot.record?.outcome?.summary, snapshot.display != .complete {
                Text(verbatim: summary).font(.callout).foregroundStyle(style.muted).multilineTextAlignment(.center)
            }
            if let error = snapshot.record?.error {
                Text(verbatim: error.message).font(.callout).foregroundStyle(style.danger).multilineTextAlignment(.center)
            }
            ForEach(Array((snapshot.record?.outcome?.warnings ?? []).enumerated()), id: \.offset) { _, warning in
                Text(verbatim: warning.message).font(.callout).foregroundStyle(style.warning).multilineTextAlignment(.center)
            }
            if snapshot.display == .failed || snapshot.display == .cancelled {
                Text(snapshot.resumable == true ? "Puedes reanudar desde donde quedó." : "Puedes volver a intentarlo.")
                    .font(.caption).foregroundStyle(style.muted)
            }
        }
        .frame(maxWidth: 520)
        .accessibilityElement(children: .combine)
        .onAppear {
            guard !reduceMotion else { return }
            withAnimation(.spring(duration: 0.35, bounce: 0.3)) { appeared = true }
        }
    }

    @ViewBuilder private var mark: some View {
        if style.skin == .terminal {
            Text(verbatim: terminalMark).font(.system(size: 22, weight: .bold, design: .monospaced))
        } else {
            Image(systemName: symbol)
        }
    }

    private var headline: Text {
        switch snapshot.display {
        case .complete: return snapshot.record?.outcome.map { Text(verbatim: $0.summary) } ?? Text("Listo")
        case .completeWithWarnings: return Text("Hecho, con avisos")
        case .failed: return Text("No se pudo terminar")
        case .cancelled: return Text("Cancelado")
        case .gone: return Text("La acción terminó; su resultado ya no está disponible")
        case .preparing, .running, .noSignal: return Text("Trabajando")
        }
    }

    private var symbol: String {
        switch snapshot.display {
        case .complete: return "checkmark.circle"
        case .completeWithWarnings: return "exclamationmark.triangle"
        case .failed: return "xmark.octagon"
        case .cancelled: return "stop.circle"
        default: return "questionmark.circle"
        }
    }

    private var terminalMark: String {
        switch snapshot.display {
        case .complete: return "[ OK ]"
        case .completeWithWarnings: return "[WARN]"
        case .failed: return "[FAIL]"
        case .cancelled: return "[STOP]"
        default: return "[ ?? ]"
        }
    }

    private var tint: Color {
        switch snapshot.display {
        case .complete: return style.success
        case .completeWithWarnings: return style.warning
        case .failed: return style.danger
        default: return style.muted
        }
    }
}

/// Size 2, compact: a floating panel with one row per action, the newest on top, for when the action runs and
/// the person is in another view or in the compact window. A click on a row asks the app to expand it to the
/// screen; it comes back to compact when the person leaves that screen, and the action never stops for it.
public struct ActionProgressCompact: View {
    @Environment(\.actionProgressStyle) private var style
    let actions: [ActionSnapshot]
    let expand: (String) -> Void

    /// `actions` newest first; `expand` gets the `progress_id` of the row clicked.
    public init(_ actions: [ActionSnapshot], expand: @escaping (String) -> Void) {
        self.actions = actions
        self.expand = expand
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            ForEach(Array(actions.enumerated()), id: \.offset) { _, snapshot in
                Button { if let id = snapshot.record?.progressID { expand(id) } } label: { row(snapshot) }
                    .buttonStyle(.plain)
                    .accessibilityHint(Text("Abre la acción en pantalla"))
            }
        }
        .padding(12)
        .frame(width: 320)
        .background(style.raised, in: RoundedRectangle(cornerRadius: CGFloat(style.skin.panelRadius)))
        .overlay(RoundedRectangle(cornerRadius: CGFloat(style.skin.panelRadius)).stroke(style.line.opacity(0.6), lineWidth: 1))
    }

    private func row(_ snapshot: ActionSnapshot) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack {
                Text(verbatim: snapshot.record?.label ?? snapshot.record?.action ?? "…")
                    .font(.callout.weight(.medium)).foregroundStyle(style.ink).lineLimit(1).truncationMode(.middle)
                Spacer(minLength: 8)
                if let percent = snapshot.percent {
                    Text(verbatim: "\(percent) %").font(.caption.monospacedDigit()).foregroundStyle(style.muted)
                }
            }
            if snapshot.display.isFinished {
                ActionClosingLine(snapshot: snapshot)
            } else {
                ActionProgressBar(fraction: snapshot.fraction, height: style.skin == .win9x ? 12 : 6)
                ActionStatusLine(snapshot: snapshot).font(.caption)
            }
        }
        .contentShape(Rectangle())
    }
}

/// The closing in one line, for the compact panel and the indicator.
struct ActionClosingLine: View {
    @Environment(\.actionProgressStyle) private var style
    let snapshot: ActionSnapshot
    var body: some View {
        switch snapshot.display {
        case .complete: Text("Listo").font(.caption).foregroundStyle(style.success)
        case .completeWithWarnings: Text("Hecho, con avisos").font(.caption).foregroundStyle(style.warning)
        case .failed: Text("No se pudo terminar").font(.caption).foregroundStyle(style.danger)
        case .cancelled: Text("Cancelado").font(.caption).foregroundStyle(style.muted)
        default: Text("Terminó").font(.caption).foregroundStyle(style.muted)
        }
    }
}

/// Size 3, the indicator: a chip for the title bar with the percent of the action, or how many run. A click
/// opens the compact panel. Background actions (a scheduled backup) use only this size; the Dock badge is
/// `ActionDockBadge`.
public struct ActionProgressIndicator: View {
    @Environment(\.actionProgressStyle) private var style
    let actions: [ActionSnapshot]
    let open: () -> Void

    public init(_ actions: [ActionSnapshot], open: @escaping () -> Void) {
        self.actions = actions
        self.open = open
    }

    public var body: some View {
        if !actions.isEmpty {
            Button(action: open) {
                HStack(spacing: 6) {
                    ActionProgressBar(fraction: lead.fraction, height: 4).frame(width: style.skin == .terminal ? 54 : 36)
                    Text(verbatim: caption).font(.caption.monospacedDigit()).foregroundStyle(style.ink).fixedSize()
                }
                .fixedSize()
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(style.raised.opacity(0.9), in: style.controlShape)
            }
            .buttonStyle(.plain)
            .help(Text("Acciones en curso"))
            .accessibilityLabel(Text("Acciones en curso"))
            .accessibilityValue(Text(verbatim: caption))
        }
    }

    private var running: [ActionSnapshot] { actions.filter { !$0.display.isFinished } }
    private var lead: ActionSnapshot { running.first ?? actions[0] }
    private var caption: String {
        if running.count > 1 { return String(localized: "\(running.count) acciones") }
        if let percent = lead.percent { return "\(percent) %" }
        return lead.display.isFinished ? "✓" : "…"
    }
}

/// The percent of the running action on the app's Dock icon, cleared when nothing runs.
@MainActor
public enum ActionDockBadge {
    public static func show(_ actions: [ActionSnapshot]) {
        let running = actions.filter { !$0.display.isFinished }
        let label: String? = switch running.count {
        case 0: nil
        case 1: running[0].percent.map { "\($0) %" } ?? "…"
        default: "\(running.count)"
        }
        if NSApplication.shared.dockTile.badgeLabel != label {
            NSApplication.shared.dockTile.badgeLabel = label
        }
    }
}
#endif
