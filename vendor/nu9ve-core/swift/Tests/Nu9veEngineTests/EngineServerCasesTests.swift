import Foundation
import Testing
@testable import Nu9veEngine

/// The 20 engine cases of data/engine-examples-v1.json, sent raw through the client's own
/// transport (bounded reading, envelope and id checks) to an engine.
enum ServerCases {
    static func run(_ configuration: EngineConfiguration) async throws {
        let connection = try EngineConnection.launch(configuration)
        defer { connection.shutdown() }
        let cases = EngineExamples.list("server")
        #expect(cases.count == 20)
        for item in cases {
            var request = item.text("request")
            if let pad = item["pad_bytes"]?.numberValue?.int {
                request = request.replacingOccurrences(of: "<pad>", with: String(repeating: "a", count: pad))
            }
            try await check(connection, item.text("name"), request, item["response"] ?? .null)
            if let then = item["then"] {
                try await check(connection, item.text("name") + " (then)", then.text("request"), then["response"] ?? .null)
            }
        }
    }

    static func check(_ connection: EngineConnection, _ name: String, _ request: String, _ want: EngineValue) async throws {
        if want.isNull {
            // No response: the next one must be the next request's.
            connection.writeRaw(Data(request.utf8))
            return
        }
        let envelope = try await connection.sendRaw(Data(request.utf8), timeout: .seconds(10))
        let expectedID = try #require(EngineID(try EngineValue(parsing: want.text("id"))), "\(name)")
        #expect(envelope.id == expectedID, "\(name): id \(envelope.id)")
        #expect(envelope.ok == (want["ok"]?.boolValue ?? false), "\(name): ok")
        switch envelope.body {
        case .data(let data, _):
            guard case .success = EngineInfo.parse(data) else {
                Issue.record("\(name): engine_info is not usable")
                return
            }
        case .error(let error):
            #expect(error["code"]?.stringValue == want.text("code"), "\(name): code")
        }
    }
}

@Test func twentyCasesAgainstTheFixture() async throws {
    try await ServerCases.run(Fixture.configuration())
}
