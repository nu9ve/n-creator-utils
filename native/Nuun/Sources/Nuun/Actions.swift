import Foundation
import Nu9veAction
import Nu9veActionProgress
import NuunCore
import SwiftUI

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
        monitor.start()
        return monitor
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
