import Foundation
import Testing
@testable import Nu9veEngine

/// An app's real engine, described by a JSON file that `NU9VE_ENGINE_LIVE_PROBE` names (see
/// swift/README.md). Read-only: the 20 cases, then the probe's reads, each on a fixture folder
/// the test writes in a temporary directory. Without the variable, nothing runs.
///
///     {"command": ["/abs/engine", "serve"], "protocol": "app-engine/1",
///      "reads": [{"method": "capabilities"},
///                {"method": "project_summary", "params": {"project": "$FIXTURE"},
///                 "fixture": {"project.json": {"name": "Test"}}}]}
enum LiveProbe {
    static let spec: EngineValue? = {
        guard let path = ProcessInfo.processInfo.environment["NU9VE_ENGINE_LIVE_PROBE"], !path.isEmpty,
              let data = FileManager.default.contents(atPath: path) else { return nil }
        return try? EngineValue(parsing: data)
    }()

    static func configuration() throws -> EngineConfiguration {
        let spec = try #require(spec)
        let strings = spec["command"]?.arrayValue?.compactMap(\.stringValue)
        let command = try #require(strings)
        let executable = try #require(command.first)
        var configuration = EngineConfiguration(executable: URL(fileURLWithPath: executable), arguments: Array(command.dropFirst()),
                                                expectedProtocol: spec["protocol"]?.stringValue)
        configuration.startupTimeout = .seconds(5)
        return configuration
    }

    /// Replaces the string `$FIXTURE` anywhere in `value`.
    static func substitute(_ value: EngineValue, _ folder: String) -> EngineValue {
        switch value {
        case .string("$FIXTURE"): .string(folder)
        case .array(let items): .array(items.map { substitute($0, folder) })
        case .object(let object): .object(EngineObject(object.map { ($0.key, substitute($0.value, folder)) }))
        default: value
        }
    }
}

@Test(.enabled(if: LiveProbe.spec != nil)) func twentyCasesAgainstALiveEngine() async throws {
    try await ServerCases.run(try LiveProbe.configuration())
}

@Test(.enabled(if: LiveProbe.spec != nil)) func readsOfALiveEngine() async throws {
    let engine = EngineClient(try LiveProbe.configuration())
    defer { engine.stop() }
    let info = try await engine.start()
    #expect(info.contract == EngineInfo.contract)
    let reads = LiveProbe.spec?["reads"]?.arrayValue ?? []
    for read in reads {
        let method = try #require(read["method"]?.stringValue)
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("nu9ve-engine-live-\(UUID().uuidString)").resolvingSymlinksInPath().path
        try FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(atPath: folder) }
        for (name, content) in read["fixture"]?.objectValue ?? [:] {
            try LiveProbe.substitute(content, folder).jsonData.write(to: URL(fileURLWithPath: folder).appendingPathComponent(name))
        }
        let params = LiveProbe.substitute(read["params"] ?? .object([:]), folder).objectValue ?? [:]
        let outcome = try await engine.call(method, params)
        switch outcome {
        case .value(let reply):
            #expect(!reply.bytes.isEmpty, "\(method)")
            if let field = read["expect_field"]?.stringValue { #expect(reply.value[field] != nil, "\(method): \(field)") }
        case .notPorted(let missing):
            Issue.record("\(method): not_ported \(missing.reason)")
        }
    }
}
