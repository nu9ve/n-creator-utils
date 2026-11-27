#if os(macOS)
import AppKit
import Foundation
import Nu9veAction
import Nu9veActionProgress
import Nu9veTheme
import SwiftUI
import Testing

// The views build and lay out in every skin, running and in each closing. What they look like is checked
// by hand in the app that adopts them; here, that nothing traps and that the monitor stops at the end.

private func record(_ state: String, extra: String = "") -> Data {
    let running = state == "running"
    return Data("""
    {"contract":"nu9ve-action/1","progress_id":"a","action":"copy","label":"Copiando finales","state":"\(state)",\
    "stage":"copying","overall_done":\(running ? 40 : 100),"overall_total":100,"unit":"bytes","files_done":4,"files_total":10,\
    \(running ? "\"current\":{\"name\":\"C0001.MP4\",\"from\":\"A\",\"to\":\"B\",\"action\":\"copy\",\"file_done\":5,\"file_total\":10}," : "")\
    "log":[{"at":"2026-11-26T18:00:01Z","kind":"copied","name":"C0000.MP4","target":"B"}],"cancellable":\(running),\
    "started_at":"2026-11-26T18:00:00Z","progressed_at":"2026-11-26T18:00:02Z","updated_at":"2026-11-26T18:00:02Z"\(extra)}
    """.utf8)
}

private let finished: [(String, String)] = [
    ("complete", ",\"outcome\":{\"summary\":\"10 archivos\",\"warnings\":[]}"),
    ("complete", ",\"outcome\":{\"summary\":\"9 archivos\",\"warnings\":[{\"code\":\"changed\",\"message\":\"1 cambió\"}]}"),
    ("failed", ",\"error\":{\"code\":\"disk_full\",\"message\":\"Lleno\"},\"resumable\":true"),
    ("cancelled", ",\"resumable\":false"),
]

private func snapshots() throws -> [ActionSnapshot] {
    let now = try #require(actionInstant("2026-11-26T18:00:03Z"))
    var shown: [ActionSnapshot] = []
    var running = ActionWatch(progressID: "a")
    shown.append(running.read(record("running"), at: now))
    for (state, extra) in finished {
        var watch = ActionWatch(progressID: "a")
        shown.append(watch.read(record(state, extra: extra), at: now))
    }
    return shown
}

@MainActor
@Test func everyViewLaysOutInEverySkin() throws {
    let all = try snapshots()
    #expect(all.map(\.display) == [.running, .complete, .completeWithWarnings, .failed, .cancelled])
    for skin in ThemeSkin.allCases {
        let style = ActionProgressStyle(skin: skin, roles: ThemeRoles(skin: skin, dark: false, base: ThemeColor(hex: "#2F78D6")!))
        for snapshot in all {
            let views: [AnyView] = [
                AnyView(ActionProgressScreen(snapshot, note: "Si cancelas, nada se borra.")),
                AnyView(ActionProgressCompact([snapshot]) { _ in }),
                AnyView(ActionProgressIndicator([snapshot]) {}),
            ]
            for view in views {
                let host = NSHostingView(rootView: view.environment(\.actionProgressStyle, style))
                host.frame = NSRect(x: 0, y: 0, width: 640, height: 480)
                host.layoutSubtreeIfNeeded()
                #expect(host.fittingSize.width > 0, "\(skin) \(snapshot.display)")
            }
        }
    }
}

@MainActor
@Test func theMonitorStopsWhenTheActionEnds() async throws {
    let replies = Replies([record("running"), record("complete", extra: finished[0].1)])
    let now = try #require(actionInstant("2026-11-26T18:00:03Z"))
    let monitor = ActionMonitor(progressID: "a", clock: { now }) { _ in await replies.next() }
    await monitor.readOnce()
    #expect(monitor.snapshot.display == .running)
    await monitor.readOnce()
    #expect(monitor.snapshot.display == .complete)
    #expect(monitor.lastFetchError == nil)
}

@MainActor
@Test func aFailedFetchKeepsTheLastSnapshot() async throws {
    struct Down: Error {}
    let monitor = ActionMonitor(progressID: "a") { _ in throw Down() }
    await monitor.readOnce()
    #expect(monitor.snapshot.display == .preparing)
    #expect(monitor.lastFetchError != nil)
}

private actor Replies {
    var queue: [Data]
    init(_ queue: [Data]) { self.queue = queue }
    func next() -> Data { queue.count > 1 ? queue.removeFirst() : queue[0] }
}
#endif
