import Foundation
import Testing
import Nu9veScore

// Fast checks in debug. That both revisions sound exactly as the engines they come from is checked in
// Release by scripts/check-score.sh (debug builds round differently).

private func manifest(_ extra: String = "", schema: String = "test-score-v1") -> Data {
    Data("""
    {"schema":"\(schema)","sample_rate":48000,"duration":0.3,"seed":3,
     "instruments":[
      {"id":"kick","kind":"drum","stem":"drums","gain_db":-6,"params":{"kind":"kick"},"sends":{"room":0.3}},
      {"id":"bass","kind":"bass","stem":"bass","gain_db":-6,"params":{"mode":"808"},"insert":{"drive":0.8}}],
     "events":[[0,0.2,0,36,0.9,0],[0.05,0.2,1,40,0.8,0]],
     "master":{"gain_db":-6}\(extra)}
    """.utf8)
}

private func render(_ data: Data, revision: ScoreRevision) throws -> Data {
    let parsed = try ScoreManifest.parse(data, schemas: ["test-score-v1"], defaultRevision: revision)
    let out = FileManager.default.temporaryDirectory.appendingPathComponent("nu9ve-score-\(UUID().uuidString).wav")
    defer { try? FileManager.default.removeItem(at: out) }
    let report = try ScoreEngine(manifest: parsed, reportSchema: "test-render-v1").render(output: out.path, from: 0, to: nil, stemsDirectory: nil)
    #expect(report.schema == "test-render-v1")
    #expect(report.nonFinite == 0)
    return try Data(contentsOf: out).dropFirst(44)
}

@Test func theRevisionComesFromTheManifestOrTheAppsDefault() throws {
    #expect(try ScoreManifest.parse(manifest(), schemas: ["test-score-v1"], defaultRevision: .v1).revision == .v1)
    #expect(try ScoreManifest.parse(manifest(), schemas: ["test-score-v1"], defaultRevision: .v2).revision == .v2)
    #expect(try ScoreManifest.parse(manifest(#","engine_revision":2"#), schemas: ["test-score-v1"], defaultRevision: .v1).revision == .v2)
    #expect(throws: ScoreError.self) { try ScoreManifest.parse(manifest(#","engine_revision":3"#), schemas: ["test-score-v1"], defaultRevision: .v1) }
    #expect(throws: ScoreError.self) { try ScoreManifest.parse(manifest(#","engine_revision":1.5"#), schemas: ["test-score-v1"], defaultRevision: .v1) }
}

@Test func onlyTheAppsSchemasAreRead() {
    #expect(throws: ScoreError.self) { try ScoreManifest.parse(manifest(schema: "other-v1"), schemas: ["test-score-v1"], defaultRevision: .v1) }
}

@Test func aRenderIsTheSameEveryTime() throws {
    #expect(try render(manifest(), revision: .v2) == render(manifest(), revision: .v2))
}

/// The kick, the reverb's low cut and the drive's make-up differ between revisions.
@Test func theRevisionsSoundDifferent() throws {
    #expect(try render(manifest(), revision: .v1) != render(manifest(), revision: .v2))
}

@Test func roleLevelsChangeTheRender() throws {
    let quieter = manifest(#","level":{"rate":20,"values_db":[-12,-12,-12,-12,-12,-12,-12,-12]}"#)
    #expect(try render(manifest(), revision: .v1) != render(quieter, revision: .v1))
}

/// What came behind new parameters leaves a manifest that does not ask for it untouched.
@Test func aNeutralStemChainLeavesTheRenderUntouched() throws {
    let neutral = manifest(#","stem_fx":{"drums":{},"bass":{"width":1,"gain_db":0}},"space_duck":0"#)
    #expect(try render(manifest(), revision: .v2) == render(neutral, revision: .v2))
    let quieter = manifest(#","stem_fx":{"bass":{"gain_db":-6}}"#)
    #expect(try render(manifest(), revision: .v2) != render(quieter, revision: .v2))
}

@Test func stemFXNamesAKnownStem() {
    #expect(throws: ScoreError.self) {
        try ScoreManifest.parse(manifest(#","stem_fx":{"vocals":{"gain_db":-3}}"#), schemas: ["test-score-v1"], defaultRevision: .v2)
    }
}
