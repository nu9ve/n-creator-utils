import CoreGraphics
import Foundation
import Testing
@testable import NuunCore

@Test func noDestinationNoPopup() {
    #expect(!ProtocolBoundary.shouldOfferCard(destinationsOnline: false, sourceIsDestination: false, removableExternal: true))
    #expect(!ProtocolBoundary.shouldOfferCard(destinationsOnline: true, sourceIsDestination: true, removableExternal: true))
    #expect(ProtocolBoundary.shouldOfferCard(destinationsOnline: true, sourceIsDestination: false, removableExternal: true))
}
@Test func archiveOnlyNeverRequiresConnectedWorkOrCreatesPending() {
    let params = IngestPolicy.parameters(archiveOnly: true, skipWork: true, deferWork: true, work: "EDICION")
    #expect(params["archive_only"] == .bool(true))
    #expect(params["confirm_archive_only"] == .bool(true))
    #expect(params["work_decision"] == .string("skipped"))
    #expect(params["work"] == nil)
}
@Test func unavailableWorkCanRemainPendingAfterConfirmation() {
    let params = IngestPolicy.parameters(archiveOnly: true, skipWork: false, deferWork: true, work: "EDICION")
    #expect(params["work_decision"] == .string("pending"))
    #expect(params["work"] == .string("EDICION"))
    let normal = IngestPolicy.parameters(archiveOnly: false, skipWork: false, deferWork: true, work: "EDICION")
    #expect(normal["archive_only"] == .bool(false))
    #expect(normal["work"] == .string("EDICION"))
}
@Test func jsonBoundaryPreservesReviewedPlan() throws {
    let raw = Data(#"{"ok":true,"data":{"digest":"abc","cleanup":true,"paths":["a b.mov"]}}"#.utf8)
    let result = try ProtocolBoundary.response(raw)
    #expect(result["cleanup"].bool)
    #expect(result["digest"].string == "abc")
    #expect(result["paths"].array.first?.string == "a b.mov")
    let roundtrip = try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(result))
    #expect(result == roundtrip)
}
@Test func errorsAreExplicit() {
    #expect(throws: BackendFailure.self) {
        try ProtocolBoundary.response(Data(#"{"ok":false,"error":{"code":"confirmation_required","message":"Confirmar"}}"#.utf8))
    }
}

@Test func nanosecondsRemainExact() throws {
    let raw = Data(#"{"ok":true,"data":{"mtime_ns":1761400000123456789}}"#.utf8)
    let value = try ProtocolBoundary.response(raw)
    let serialized = try JSONEncoder().encode(value)
    #expect(String(decoding: serialized, as: UTF8.self).contains("1761400000123456789"))
}
@Test(.enabled(if: builtEngine != nil)) func realEngineBoundaryUsesIsolatedCatalog() async throws {
    let state = URL(fileURLWithPath: "/private/tmp").appendingPathComponent("media-swift-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let client = BackendClient(configuration: .init(stateDirectory: state.path, engine: builtEngine))
    let result = try await client.call("suggest", ["source": .string("/does/not/exist")])
    #expect(!result["show"].bool)
    #expect(result["reason"].string == "no_registered_destinations")
}

@Test func unsignedVolumeIdentityRemainsExact() throws {
    let raw = Data(#"{"ok":true,"data":{"root_inode":18446744073709551615}}"#.utf8)
    let value = try ProtocolBoundary.response(raw)
    #expect(value["root_inode"] == .unsignedInteger(UInt64.max))
    #expect(String(decoding: try JSONEncoder().encode(value), as: UTF8.self).contains("18446744073709551615"))
}

/// The transport of a reviewed plan document, against a fixture engine (`tests/fixtures/native_plan_backend.py`,
/// behind a one-line wrapper) whose card has a 64-bit root inode.
@Test func originalReviewedDocumentSurvivesNativePlanApplyRoundtrip() async throws {
    let state = URL(fileURLWithPath: "/private/tmp").appendingPathComponent("media-plan-swift-" + UUID().uuidString)
    try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let wrapper = state.appendingPathComponent("fixture-engine")
    try "#!/bin/sh\nexec /usr/bin/python3 '\(repoRoot.appendingPathComponent("tests/fixtures/native_plan_backend.py").path)' \"$@\"\n"
        .write(to: wrapper, atomically: true, encoding: .utf8)
    try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: wrapper.path)
    let client = BackendClient(configuration: .init(stateDirectory: state.path, engine: wrapper.path))
    let document = try await client.call("plan_document", ["source": .string(state.appendingPathComponent("card").path),
        "archive": .string("MEDIA"), "work": .string("EDICION"), "project": .string("roundtrip"), "cleanup": .bool(false)]).string
    let reviewed = try ReviewedPlan(document: document)
    #expect(document.contains("1.0"))
    #expect(reviewed.display["source_identity"]["root_inode"] == .unsignedInteger(UInt64.max))
    // The old native path reconstructs integral floats as ints and must still fail closed.
    do {
        _ = try await client.call("apply", ["plan": reviewed.display, "digest": reviewed.display["digest"]])
        Issue.record("Re-encoded numeric plan unexpectedly passed its original digest")
    } catch let error as BackendFailure {
        #expect(error.code == "plan_changed")
    }
    #expect(try await client.call("jobs").array.isEmpty)
    let result = try await client.call("apply", reviewed.applyParameters)
    #expect(result["state"].string == "source_retained")
    #expect(result["progress"]["verified_copies"].double == 2)
    #expect(FileManager.default.fileExists(atPath: state.appendingPathComponent("card/photo.JPG").path))
    // A changed document is rejected even when the caller supplies the original digest.
    let changed = document.replacingOccurrences(of: "roundtrip", with: "changed-project")
    do {
        _ = try await client.call("apply", ["plan": .string(changed), "digest": reviewed.display["digest"]])
        Issue.record("Tampered plan unexpectedly applied")
    } catch let error as BackendFailure { #expect(error.code == "plan_changed") }
}

@Test(.enabled(if: builtEngine != nil)) func persistentReaderAnswersReadsInOrderAndRefusesMutations() async throws {
    let state = URL(fileURLWithPath: "/private/tmp").appendingPathComponent("media-reader-swift-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let reader = PersistentReader(configuration: .init(stateDirectory: state.path, engine: builtEngine))
    defer { reader.stop() }
    async let first = reader.call("snapshot", ["cards": .bool(false)])
    async let second = reader.call("suggest", ["source": .string("/does/not/exist")])
    let (snapshot, suggestion) = try await (first, second)
    #expect(snapshot["volumes"].array.isEmpty)
    #expect(snapshot["defaults"] == .object([:]))  // a fresh library has no defaults; the code adds none
    #expect(suggestion["reason"].string == "no_registered_destinations")
    do {
        _ = try await reader.call("defaults", ["project": .string("x")])
        Issue.record("A mutation unexpectedly reached the read-only server")
    } catch let error as BackendFailure { #expect(error.code == "read_only") }
    // After stopping, the next read restarts the child transparently.
    reader.stop()
    #expect(try await reader.call("jobs").array.isEmpty)
}

@Test func treemapFillsTheBoundsProportionallyAndKeepsInputOrder() {
    let bounds = CGRect(x: 10, y: 20, width: 600, height: 300)
    let values = [5.0, 120, 0, 40, 40, 300, 1]
    let rects = Treemap.layout(values, in: bounds)
    #expect(rects.count == values.count)
    #expect(rects[2] == .zero)
    let scale = Double(bounds.width * bounds.height) / values.reduce(0, +)
    for (value, rect) in zip(values, rects) where value > 0 {
        #expect(abs(Double(rect.width * rect.height) - value * scale) < 0.01)
        #expect(bounds.insetBy(dx: -0.01, dy: -0.01).contains(rect))
    }
    for first in rects.indices where values[first] > 0 {
        for second in rects.indices where second > first && values[second] > 0 {
            #expect(rects[first].insetBy(dx: 0.01, dy: 0.01).intersection(rects[second].insetBy(dx: 0.01, dy: 0.01)).isEmpty)
        }
    }
    #expect(Treemap.layout([], in: bounds).isEmpty)
    #expect(Treemap.layout([1, 2], in: .zero) == [.zero, .zero])
}
@Test(.enabled(if: builtEngine != nil)) func diskIndexCrossesTheRealBoundary() async throws {
    let scratch = URL(fileURLWithPath: "/private/tmp")
    let state = scratch.appendingPathComponent(UUID().uuidString)
    let folder = scratch.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state); try? FileManager.default.removeItem(at: folder) }
    try FileManager.default.createDirectory(at: folder.appendingPathComponent("Music"), withIntermediateDirectories: true)
    try Data(count: 2 * 1024 * 1024).write(to: folder.appendingPathComponent("Music/song.wav"))
    let configuration = BackendConfiguration(stateDirectory: state.path, engine: builtEngine)
    let client = BackendClient(configuration: configuration), reader = PersistentReader(configuration: configuration)
    defer { reader.stop() }
    _ = try await client.call("register_volume", ["root": .string(folder.path), "name": .string("QA"), "role": .string("work"), "allow_local": .bool(true)])
    let scan = try await client.call("storage_scan", ["target": .string("QA")])
    #expect(scan["state"].string == "finished")
    #expect(scan["files"].double == 1)
    let overview = try await reader.call("storage_overview")
    #expect(overview["targets"].array.first?["indexed"].bool == true)
    let tree = try await reader.call("storage_tree", ["target": .string("QA")])
    #expect(tree["folders"].array.first?["name"].string == "Music")
    await #expect(throws: BackendFailure.self) { _ = try await reader.call("storage_scan", ["target": .string("QA")]) }
}
@Test func timestampsReadEngineAndManifestFormats() {
    let instant = Date(timeIntervalSince1970: 1_793_127_106)
    #expect(Timestamps.parse("2026-10-27T18:51:46.123456Z") == instant)
    #expect(Timestamps.parse("2026-10-27T11:51:46-07:00") == instant)
    #expect(Timestamps.parse("2026-11-01T17:20-07:00") != nil)
    #expect(Timestamps.parse("") == nil)
    #expect(Timestamps.label("no es fecha") == "no es fecha")
}

@Test func flowBlocksStayCenteredAndFitTheirStage() {
    #expect(FlowGeometry.centers(count: 0, blockHeight: 100, spacing: 10, height: 210).isEmpty)
    #expect(FlowGeometry.centers(count: 1, blockHeight: 100, spacing: 10, height: 210) == [105])
    #expect(FlowGeometry.centers(count: 2, blockHeight: 100, spacing: 10, height: 210) == [50, 160])
    #expect(FlowGeometry.blockWidth(available: 1000, stages: 5, connector: 30, range: 120...210) == 176)
    #expect(FlowGeometry.blockWidth(available: 300, stages: 5, connector: 30, range: 120...210) == 120)
    #expect(FlowGeometry.blockWidth(available: 3000, stages: 3, connector: 30, range: 120...210) == 210)
}

// MARK: One engine (F5 (e), 2026-11-27): Rust answers everything, without Python behind it

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()

/// The engine this checkout built (`cargo build -p nuun-engine`), the debug one first: the gate builds it.
private let builtEngine: String? = ["debug", "release"]
    .map { repoRoot.appendingPathComponent("engine/target/\($0)/nuun-engine").path }
    .first { FileManager.default.isExecutableFile(atPath: $0) }

/// A stand-in engine: `rpc-serve` keeps `snapshot` and `jobs`, `resolve` is a one-shot read, `defaults` a write.
private let fakeEngine = #"""
#!/usr/bin/python3
import json, sys
serve = 'rpc-serve' in sys.argv
for line in sys.stdin:
    request = json.loads(line)
    method = request['method']
    if method == 'engine_info':
        out = {'ok': True, 'data': {'engine': 'rust', 'ported': ['snapshot', 'jobs'], 'once': ['resolve'], 'writes': ['defaults']}}
    elif serve and method == 'snapshot':
        out = {'ok': True, 'data': {'from': 'rpc-serve'}}
    elif serve and method == 'jobs':
        out = {'ok': False, 'error': {'code': 'unknown_project', 'message': 'Proyecto desconocido'}}
    elif serve:
        out = {'ok': False, 'error': {'code': 'read_only', 'message': 'rpc-serve only answers reads: ' + method}}
    elif method == 'resolve':
        out = {'ok': True, 'data': {'from': 'rpc'}}
    elif method == 'defaults':
        out = {'ok': True, 'data': {'project': 'from-rust'}}
    else:
        sys.exit(3)
    out['id'] = request.get('id')
    sys.stdout.write(json.dumps(out) + '\n'); sys.stdout.flush()
"""#

private func fakeEngineFolder() throws -> (URL, String) {
    let scratch = URL(fileURLWithPath: "/private/tmp").appendingPathComponent("media-engine-swift-" + UUID().uuidString)
    try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
    let engine = scratch.appendingPathComponent("fake-engine")
    try fakeEngine.write(to: engine, atomically: true, encoding: .utf8)
    try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: engine.path)
    return (scratch, engine.path)
}

@Test func theReaderAsksTheEngineAndNeverStartsAWrite() async throws {
    let (scratch, engine) = try fakeEngineFolder()
    defer { try? FileManager.default.removeItem(at: scratch) }
    let reader = PersistentReader(configuration: .init(stateDirectory: scratch.appendingPathComponent("state").path, engine: engine))
    defer { reader.stop() }
    #expect(try await reader.call("snapshot")["from"].string == "rpc-serve")
    #expect(reader.answeredBy["snapshot"] == "rpc-serve")
    // A one-shot read the server does not keep: its own `rpc`.
    #expect(try await reader.call("resolve", ["asset_id": .string("a1")])["from"].string == "rpc")
    #expect(reader.answeredBy["resolve"] == "rpc")
    // An error of the method itself is the answer; there is nobody else to ask.
    do {
        _ = try await reader.call("jobs")
        Issue.record("The engine's method error was not returned")
    } catch let failure as BackendFailure { #expect(failure.code == "unknown_project") }
    // A write goes to the server, which refuses it.
    do {
        _ = try await reader.call("defaults", ["project": .string("x")])
        Issue.record("A write reached the one-shot engine through the reader")
    } catch let failure as BackendFailure { #expect(failure.code == "read_only") }
}

@Test func oneShotCallsGoToTheEngineOnly() async throws {
    let (scratch, engine) = try fakeEngineFolder()
    defer { try? FileManager.default.removeItem(at: scratch) }
    let client = BackendClient(configuration: .init(stateDirectory: scratch.appendingPathComponent("state").path, engine: engine))
    #expect(try await client.call("defaults", ["project": .string("x")])["project"].string == "from-rust")
    #expect(try await client.call("resolve", ["asset_id": .string("a1")])["from"].string == "rpc")
    // The engine ends without an answer: that is the failure, not a cue to ask someone else.
    do {
        _ = try await client.call("apply")
        Issue.record("The fake engine terminates without an apply response")
    } catch let failure as BackendFailure { #expect(failure.code == "backend_failed") }
}

@Test func withoutAnEngineNothingAnswers() async throws {
    let missing = BackendConfiguration(engine: "/nonexistent/nuun-engine")
    #expect(!missing.usable)
    await #expect(throws: BackendFailure.self) { _ = try await BackendClient(configuration: missing).call("jobs") }
    await #expect(throws: BackendFailure.self) { _ = try await PersistentReader(configuration: missing).call("jobs") }
}

/// The one-pass parser gives the same values as the Codable init, on edge cases and on the whole golden file.
@Test func fastParserMatchesCodable() throws {
    let samples = [
        #"{"a":1,"b":1.5,"c":-3,"d":true,"e":false,"f":null,"g":"x","h":[1,2.0,"y",{"z":[]}],"i":18446744073709551615,"j":1e3,"k":0}"#,
        #"[9223372036854775807,-9223372036854775808,0.1,"é",{}]"#,
        #""fragment""#, "42", "true", "null",
    ]
    for sample in samples {
        let data = Data(sample.utf8)
        #expect(try JSONValue.parse(data) == JSONDecoder().decode(JSONValue.self, from: data), "\(sample)")
    }
    let golden = repoRoot.appendingPathComponent("tests/fixtures/engine-golden/recipe.json")
    let data = try Data(contentsOf: golden)
    #expect(try JSONValue.parse(data) == JSONDecoder().decode(JSONValue.self, from: data))
}

/// Timing only, on a reply saved by hand (`NUUN_REPLY_SAMPLE=<file>`); skipped otherwise.
@Test(.enabled(if: ProcessInfo.processInfo.environment["NUUN_REPLY_SAMPLE"] != nil))
func replyDecodingTiming() throws {
    let data = try Data(contentsOf: URL(fileURLWithPath: ProcessInfo.processInfo.environment["NUUN_REPLY_SAMPLE"]!))
    let clock = ContinuousClock()
    var fast = JSONValue.null, slow = JSONValue.null
    let fastTime = try clock.measure { fast = try JSONValue.parse(data) }
    let slowTime = try clock.measure { slow = try JSONDecoder().decode(JSONValue.self, from: data) }
    print("reply \(data.count) bytes: parse \(fastTime), codable \(slowTime)")
    #expect(fast == slow)
}

@Test func theCompactWindowStartsRightOfCenterAndInsideTheScreen() {
    let screen = CGRect(x: 0, y: 25, width: 1512, height: 920)
    let frame = WindowPlacement.compact(width: 440, height: 700, in: screen)
    #expect(abs(frame.midX - 1134) < 0.01)              // three quarters of the width
    #expect(frame.midY > screen.midY)                   // a little above the middle
    #expect(screen.contains(frame))
    // A small or offset screen still holds it whole.
    let small = CGRect(x: -1280, y: 0, width: 600, height: 500)
    let fitted = WindowPlacement.compact(width: 440, height: 700, in: small)
    #expect(small.contains(fitted) && fitted.height == 500)
}

@Test func aSecondWorkDiskOpensTheCompactOnlyForChangesNotDeclined() throws {
    let changed = try JSONValue.parse(Data(#"""
    {"key": "work_copy_changed:A:B", "flow": "work_copy_changed", "count": 2, "bytes": 30,
     "source": {"root": "/Volumes/ESSD"}, "destination": {"root": "/Volumes/SUBSSD"},
     "paths": [{"path": "atlas/b.json"}, {"path": "atlas/a.json"}]}
    """#.utf8))
    let other = try JSONValue.parse(Data(#"{"key": "conversations_backup:X", "flow": "conversations_backup", "count": 9}"#.utf8))
    // The arriving disk is either end of the pair; another disk, or another flow, opens nothing.
    #expect(WorkCopyPrompt.toOpen([other, changed], arrived: "/Volumes/SUBSSD", declined: [:]) == changed)
    #expect(WorkCopyPrompt.toOpen([changed], arrived: "/Volumes/ESSD/", declined: [:]) == changed)
    #expect(WorkCopyPrompt.toOpen([changed, other], arrived: "/Volumes/MEDIA9", declined: [:]) == nil)
    // «Ahora no» to these changes: not again. New changes on the same pair: again.
    let declined = [changed["key"].string: WorkCopyPrompt.signature(changed)]
    #expect(WorkCopyPrompt.toOpen([changed], arrived: "/Volumes/SUBSSD", declined: declined) == nil)
    var grown = changed
    if case .object(var fields) = grown { fields["count"] = .integer(3); fields["paths"] = .array(changed["paths"].array + [.object(["path": .string("atlas/c.json")])]); grown = .object(fields) }
    #expect(WorkCopyPrompt.toOpen([grown], arrived: "/Volumes/SUBSSD", declined: declined) == grown)
    // The order the engine lists paths in does not change the signature.
    var reordered = changed
    if case .object(var fields) = reordered { fields["paths"] = .array(changed["paths"].array.reversed()); reordered = .object(fields) }
    #expect(WorkCopyPrompt.signature(reordered) == WorkCopyPrompt.signature(changed))
}
