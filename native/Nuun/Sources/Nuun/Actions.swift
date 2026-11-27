import Foundation
import Nu9veAction
import Nu9veActionProgress
import NuunCore
import SwiftUI
import AppKit
import UserNotifications

// nu9ve-action-v1 (F4c, 2026-11-27): every long action the app launches gives the engine a `progress_id`
// and reads its record with `action_progress` through the core's `ActionMonitor`. The view is the core's
// `ActionProgressScreen`; this file only says which action fills which place of the app.

/// Where an action is shown. Each place has at most one action at a time, so a backup of conversations and a
/// release never overwrite each other's bar.
enum ActionSlot: Hashable, CaseIterable {
    /// A release, a delivery from the archive or a copy between work disks.
    case action
    /// The conversations backup (Ajustes → Conversaciones and the Home card).
    case conversations
    /// The review and the apply of a project closeout.
    case closure
    /// The ingest of a card or a folder.
    case ingest
}

/// A reader that is not there: the monitor keeps «Preparando» and tries again on its next read.
struct ActionReaderUnavailable: Error {}

/// A new id for an action (§1): lower case, as the pattern asks.
func newProgressID() -> String { UUID().uuidString.lowercased() }

extension Model {
    /// Starts reading the action `id` in `slot`, replacing what the slot showed.
    @discardableResult
    func startAction(_ slot: ActionSlot, _ id: String) -> ActionMonitor {
        actionMonitors[slot]?.stop()
        let reader = self.reader
        let monitor = ActionMonitor(progressID: id) { id in
            guard let reader else { throw ActionReaderUnavailable() }
            let value = try await reader.call("action_progress", ["progress_id": .string(id)])
            return try JSONEncoder().encode(value)
        }
        actionMonitors[slot] = monitor
        actionStarted[slot] = Date()
        actionNames[slot] = slot == .action ? runningAction : ""
        monitor.start()
        return monitor
    }

    /// Every action read in the app, newest first, with the place it belongs to: the bar's indicator and the
    /// compact panel show them, finished ones with their closing until the person clears them.
    var actionsInView: [(slot: ActionSlot, snapshot: ActionSnapshot)] {
        actionMonitors.keys
            .sorted { (actionStarted[$0] ?? .distantPast) > (actionStarted[$1] ?? .distantPast) }
            .compactMap { slot in
                // A call that failed before the engine wrote anything leaves nothing to show.
                guard let monitor = actionMonitors[slot], monitor.snapshot.record != nil || monitor.isReading else { return nil }
                return (slot, monitor.snapshot)
            }
    }

    /// «Limpiar»: the finished actions leave the panel; those that run stay.
    func clearFinishedActions() {
        for (slot, monitor) in actionMonitors where monitor.snapshot.display.isFinished || !monitor.isReading {
            actionMonitors[slot] = nil
            actionStarted[slot] = nil
            actionNames[slot] = nil
        }
        ActionDockBadge.show(actionsInView.map(\.snapshot))
    }

    /// A row of the compact panel was clicked: the action's own screen, in the full window (size 1). Leaving
    /// that screen, the action is back in the panel; it never stops for it.
    func expandAction(_ progressID: String) {
        guard let slot = actionMonitors.first(where: { $0.value.progressID == progressID })?.key else { return }
        showActionPanel = false
        if presentation != .full { changeMode?(.full) }
        switch slot {
        case .ingest: route = .ingest; ingestStep = .run
        case .conversations: openSettings?("conversations")
        case .closure: showClosure = true
        case .action:
            switch actionNames[slot] ?? "" {
            case "release": showRelease = true
            case "deliver": open(.files)
            default: break  // A copy between disks keeps its sheet open while it runs.
            }
        }
    }

    /// The call that launched the action returned or failed: stop reading. The monitor also stops on its own once
    /// it reads the closing; this covers a call that failed before the engine wrote any record.
    func stopAction(_ slot: ActionSlot) {
        actionMonitors[slot]?.stop()
    }

    /// What `slot` shows: the monitor's last snapshot, or «Preparando» before the first read.
    func actionSnapshot(_ slot: ActionSlot) -> ActionSnapshot {
        actionMonitors[slot]?.snapshot ?? .preparing
    }

    /// The record's stage while it runs, for the one-line captions beside the dialogs' buttons.
    func actionStage(_ slot: ActionSlot) -> String {
        actionMonitors[slot]?.snapshot.record?.stage ?? ""
    }
}

/// The colors of the core's action views, from the app's palette.
extension ActionProgressStyle {
    init(_ palette: CreatorPalette) {
        self.init(skin: palette.skin, roles: palette.roles)
    }
}

/// A long action in one line, for a card: the core's bar and the engine's numbers.
struct ActionLine: View {
    @Environment(\.creatorPalette) private var palette
    let snapshot: ActionSnapshot
    /// What to say while there is no amount yet.
    let idle: String

    var body: some View {
        HStack(spacing: 8) {
            ActionProgressBar(fraction: snapshot.fraction).frame(width: 140)
            Text(verbatim: text).font(.caption.monospacedDigit()).foregroundStyle(palette.muted).lineLimit(1)
        }
        .accessibilityElement(children: .combine)
    }

    private var text: String {
        guard let percent = snapshot.percent, let record = snapshot.record, let total = record.overallTotal, total > 0 else { return idle }
        guard record.unit == .bytes else { return "\(percent) %" }
        return "\(percent) % · " + String(localized: "\(bytesLabel(Double(record.overallDone))) de \(bytesLabel(Double(total)))")
    }
}

#if DEBUG
extension Model {
    /// QA captures: shows a running record in `slot` as if the engine had written it, through the same monitor.
    /// `fields` adds or replaces the record's measure (`overall_done`, `current`, `log`…).
    func showQAAction(_ slot: ActionSlot, action: String, stage: String, _ fields: [String: JSONValue]) async {
        let id = "qa-\(slot)"
        let now = Date(), instant = { (date: Date) in JSONValue.string(ISO8601DateFormatter().string(from: date)) }
        var record: [String: JSONValue] = [
            "contract": .string(actionContract), "progress_id": .string(id), "state": .string("running"),
            "action": .string(action), "stage": .string(stage), "unit": .string("bytes"), "cancellable": .bool(true),
            "overall_done": .integer(0), "overall_total": .null,
            "started_at": instant(now - 60), "progressed_at": instant(now - 1), "updated_at": instant(now),
        ]
        record.merge(fields) { $1 }
        let data = (try? JSONEncoder().encode(JSONValue.object(record))) ?? Data()
        let monitor = ActionMonitor(progressID: id) { _ in data }
        actionMonitors[slot] = monitor
        await monitor.readOnce()
    }

    /// A log line of a QA record, with its instant.
    static func qaLogLine(_ kind: String, _ name: String, _ target: String) -> JSONValue {
        .object(["at": .string(ISO8601DateFormatter().string(from: Date())), "kind": .string(kind), "name": .string(name), "target": .string(target)])
    }
}
#endif

/// Size 3 in the window's bar: the core's chip with the lead action's percent (or how many run), the Dock badge,
/// and a notice when an action ends while nüün is not in front. A click opens size 2, the compact panel: one row
/// per action, newest on top; a row expands to its screen.
struct ActionBarIndicator: View {
    @Bindable var model: Model
    @Environment(\.creatorPalette) private var palette

    var body: some View {
        let actions = model.actionsInView.map(\.snapshot)
        ActionProgressIndicator(actions) { model.showActionPanel.toggle() }
            .popover(isPresented: $model.showActionPanel, arrowEdge: .bottom) {
                VStack(alignment: .trailing, spacing: 8) {
                    ActionProgressCompact(model.actionsInView.map(\.snapshot)) { model.expandAction($0) }
                    if model.actionsInView.contains(where: { $0.snapshot.display.isFinished }) {
                        Button("Limpiar terminadas") { model.clearFinishedActions() }.controlSize(.small)
                    }
                }
                .padding(8)
                .environment(\.actionProgressStyle, ActionProgressStyle(palette))
            }
            .onAppear { ActionDockBadge.show(actions) }
            .onChange(of: actions) { before, after in
                ActionDockBadge.show(after)
                let ended = after.filter { snapshot in
                    snapshot.display.isFinished && before.contains { $0.record?.progressID == snapshot.record?.progressID && !$0.display.isFinished }
                }
                if !NSApp.isActive { ended.forEach(model.notifyActionEnded) }
            }
    }
}

extension Model {
    /// nu9ve-notifications-v1: an action ended while nüün was not in front. Its closing in one sentence.
    func notifyActionEnded(_ snapshot: ActionSnapshot) {
        guard Bundle.main.bundleIdentifier != nil, let record = snapshot.record else { return }
        let title: String = switch snapshot.display {
        case .complete: String(localized: "Listo")
        case .completeWithWarnings: String(localized: "Hecho, con avisos")
        case .failed: String(localized: "No se pudo terminar")
        case .cancelled: String(localized: "Cancelado")
        default: String(localized: "Terminó")
        }
        let body = record.error?.message ?? record.outcome?.summary ?? ""
        Task {
            let center = UNUserNotificationCenter.current()
            guard (try? await center.requestAuthorization(options: [.alert])) == true else { return }
            let content = UNMutableNotificationContent()
            content.title = title
            content.body = body
            try? await center.add(UNNotificationRequest(identifier: "action:" + record.progressID, content: content, trigger: nil))
        }
    }
}
