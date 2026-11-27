import Foundation
import Nu9veAction
import Testing

// nu9ve-action-v1 §6: data/action-examples-v1.json, written by the Python reference. The same requests,
// records and reads must give the same answer here.

private let examplesURL = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    .appendingPathComponent("data/action-examples-v1.json")

private let progressID = "ingreso-2026-11-26-1"

private struct Examples: Decodable {
    struct Request: Decodable {
        let name: String
        let request: String
        let ok: Bool
        let code: String?
    }
    struct Valid: Decodable {
        let name: String
        let record: String
    }
    struct Invalid: Decodable {
        let name: String
        let record: String
        let problem: String
    }
    struct Shows: Decodable {
        let display: String
        let percent: Int?
        let eta: String
        let resumable: Bool?
        let ignored: String?
    }
    struct Read: Decodable {
        let at: String
        let reply: String
        let shows: Shows
    }
    struct Sequence: Decodable {
        let name: String
        let reads: [Read]
        let steps: [String?]
    }
    let requests: [Request]
    let valid: [Valid]
    let invalid: [Invalid]
    let sequences: [Sequence]
}

private func examples() throws -> Examples {
    try JSONDecoder().decode(Examples.self, from: Data(contentsOf: examplesURL))
}

@Test func requestsAreReadLikeTheReference() throws {
    for example in try examples().requests {
        let request = try #require(JSONSerialization.jsonObject(with: Data(example.request.utf8)) as? [String: Any])
        let params = request["params"] as? [String: Any] ?? [:]
        #expect(actionProgressParamsProblem(params) == example.code, "\(example.name)")
        #expect((actionProgressParamsProblem(params) == nil) == example.ok, "\(example.name)")
    }
}

@Test func validRecordsAreValid() throws {
    for example in try examples().valid {
        let result = ActionRecord.read(Data(example.record.utf8), progressID: progressID)
        if case .failure(let problem) = result { Issue.record("\(example.name): \(problem.rawValue)") }
    }
}

@Test func invalidRecordsGiveTheReferenceProblem() throws {
    for example in try examples().invalid {
        switch ActionRecord.read(Data(example.record.utf8), progressID: progressID) {
        case .success: Issue.record("\(example.name): accepted")
        case .failure(let problem): #expect(problem.rawValue == example.problem, "\(example.name)")
        }
    }
}

@Test func aRecordOver256KiBIsTooLarge() throws {
    let record = try examples().valid[0].record
    let padded = record.dropLast() + ",\"x_blob\":\"" + String(repeating: "x", count: 256 * 1024) + "\"}"
    #expect(ActionRecord.read(Data(padded.utf8), progressID: progressID) == .failure(.tooLarge))
}

@Test func sequencesShowWhatTheReferenceShows() throws {
    for sequence in try examples().sequences {
        var watch = ActionWatch(progressID: progressID)
        var records: [ActionRecord?] = []
        for read in sequence.reads {
            let now = try #require(actionInstant(read.at))
            let shown = watch.read(Data(read.reply.utf8), at: now)
            #expect(shown.display.rawValue == read.shows.display, "\(sequence.name) at \(read.at)")
            #expect(shown.percent == read.shows.percent, "\(sequence.name) at \(read.at)")
            #expect(shown.eta.rawValue == read.shows.eta, "\(sequence.name) at \(read.at)")
            #expect(shown.resumable == read.shows.resumable, "\(sequence.name) at \(read.at)")
            #expect(shown.ignored?.rawValue == read.shows.ignored, "\(sequence.name) at \(read.at)")
            records.append(try? ActionRecord.read(Data(read.reply.utf8), progressID: progressID).get())
        }
        let steps = zip(records, records.dropFirst()).map { before, after -> String? in
            guard let before, let after else { return nil }
            return actionStepProblem(before, after)
        }
        #expect(steps == sequence.steps, "\(sequence.name)")
    }
}

@Test func theTimeLeftComesWithProgressAndIsSmoothed() throws {
    // Half done in 10 s: about 10 s left. Without progress for 20 s, it is pending, not a number.
    func record(_ done: Int, progressed: String, updated: String) -> Data {
        Data("""
        {"contract":"nu9ve-action/1","progress_id":"x","action":"copy","state":"running","stage":"copying",\
        "overall_done":\(done),"overall_total":100,"unit":"bytes","cancellable":true,\
        "started_at":"2026-11-26T18:00:00Z","progressed_at":"\(progressed)","updated_at":"\(updated)"}
        """.utf8)
    }
    let start = try #require(actionInstant("2026-11-26T18:00:00Z"))
    var watch = ActionWatch(progressID: "x")
    let half = watch.read(record(50, progressed: "2026-11-26T18:00:10Z", updated: "2026-11-26T18:00:10Z"), at: start + 10)
    #expect(half.eta == .shown)
    #expect(abs((half.secondsLeft ?? 0) - 10) < 0.001)
    let stalled = watch.read(record(50, progressed: "2026-11-26T18:00:10Z", updated: "2026-11-26T18:00:30Z"), at: start + 30)
    #expect(stalled.eta == .pending)
    #expect(stalled.secondsLeft == nil)
    let moving = watch.read(record(75, progressed: "2026-11-26T18:00:31Z", updated: "2026-11-26T18:00:31Z"), at: start + 31)
    #expect(moving.eta == .shown)
    // 0.3 × (31 s × 0.25 / 0.75) + 0.7 × 10 s
    #expect(abs((moving.secondsLeft ?? 0) - (0.3 * 31.0 / 3 + 0.7 * 10)) < 0.001)
}

@Test func instantsFollowTheContract() {
    #expect(actionInstant("2026-11-26T18:00:00Z") == 1_795_716_000)
    #expect(actionInstant("2026-11-26T18:00:00.5Z") == 1_795_716_000.5)
    #expect(actionInstant("2026-11-26T18:00:00+00:00") == nil)
    #expect(actionInstant("2026-02-30T18:00:00Z") == nil)
    #expect(actionInstant("2026-11-26 18:00:00Z") == nil)
}
