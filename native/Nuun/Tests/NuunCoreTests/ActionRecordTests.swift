import Foundation
import Nu9veAction
import Testing

// nu9ve-action-v1: what the Rust engine writes must pass the core's reader, or the app would stay on
// «Preparando». The records come from `engine_records_fixture` (engine/crates/nuun-media/src/action.rs),
// which fails when the engine's records and this file part ways.

private func engineRecords() throws -> [(name: String, data: Data, progressID: String)] {
    let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("Fixtures/engine-action-records.json")
    let file = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any]
    let records = file?["records"] as? [[String: Any]] ?? []
    return try records.map { entry in
        let record = entry["record"] as? [String: Any] ?? [:]
        return (entry["name"] as? String ?? "", try JSONSerialization.data(withJSONObject: record), record["progress_id"] as? String ?? "")
    }
}

@Test func engineRecordsPassTheCoreReader() throws {
    let records = try engineRecords()
    #expect(records.count == 5)
    for (name, data, progressID) in records {
        let read = ActionRecord.read(data, progressID: progressID)
        if case .failure(let problem) = read { Issue.record("\(name): \(problem)") }
    }
}

@Test func engineRecordsShowAsTheCoreViewExpects() throws {
    let records = Dictionary(uniqueKeysWithValues: try engineRecords().map { ($0.name, ($0.data, $0.progressID)) })
    func snapshot(_ name: String) -> ActionSnapshot {
        let (data, id) = records[name]!
        var watch = ActionWatch(progressID: id)
        return watch.read(data, at: actionInstant("2026-11-27T10:00:03Z")!)
    }
    let running = snapshot("running")
    #expect(running.display == .running)
    #expect(running.percent == 37)
    #expect(running.record?.current?.name == "C0002.MP4")
    #expect(snapshot("complete with a warning").display == .completeWithWarnings)
    let failed = snapshot("failed")
    #expect(failed.display == .failed)
    #expect(failed.resumable == true)
    #expect(failed.percent == 50)
    #expect(snapshot("cancelled before any file").display == .cancelled)
    #expect(snapshot("closure releasing").record?.commonStage == .releasing)
    var unknown = ActionWatch(progressID: "nope")
    let data = Data(#"{"contract":"nu9ve-action/1","progress_id":"nope","state":"unknown"}"#.utf8)
    #expect(unknown.read(data).display == .preparing)
}
