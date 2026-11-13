import CoreGraphics
import Foundation
import Testing
@testable import MediaMenuCore

@Test func noDestinationNoPopup() {
    #expect(!ProtocolBoundary.shouldOfferCard(destinationsOnline: false, sourceIsDestination: false, removableExternal: true))
    #expect(!ProtocolBoundary.shouldOfferCard(destinationsOnline: true, sourceIsDestination: true, removableExternal: true))
    #expect(ProtocolBoundary.shouldOfferCard(destinationsOnline: true, sourceIsDestination: false, removableExternal: true))
}
@Test func archiveOnlyNeverRequiresConnectedWorkOrCreatesPending() {
    let params = IngestPolicy.parameters(archiveOnly: true, skipWork: true, deferWork: true, work: "ESSD")
    #expect(params["archive_only"] == .bool(true))
    #expect(params["confirm_archive_only"] == .bool(true))
    #expect(params["work_decision"] == .string("skipped"))
    #expect(params["work"] == nil)
}
@Test func unavailableWorkCanRemainPendingAfterConfirmation() {
    let params = IngestPolicy.parameters(archiveOnly: true, skipWork: false, deferWork: true, work: "ESSD")
    #expect(params["work_decision"] == .string("pending"))
    #expect(params["work"] == .string("ESSD"))
    let normal = IngestPolicy.parameters(archiveOnly: false, skipWork: false, deferWork: true, work: "ESSD")
    #expect(normal["archive_only"] == .bool(false))
    #expect(normal["work"] == .string("ESSD"))
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
@Test func realPythonBoundaryUsesIsolatedCatalog() async throws {
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let state = URL(fileURLWithPath: "/private/tmp").appendingPathComponent("media-swift-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let client = BackendClient(configuration: .init(python: "/usr/bin/python3", script: repo.appendingPathComponent("nu9ve.py").path, stateDirectory: state.path))
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

@Test func originalReviewedDocumentSurvivesNativePlanApplyRoundtrip() async throws {
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let state = URL(fileURLWithPath: "/private/tmp").appendingPathComponent("media-plan-swift-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let client = BackendClient(configuration: .init(python: "/usr/bin/python3", script: repo.appendingPathComponent("tests/fixtures/native_plan_backend.py").path, stateDirectory: state.path))
    let document = try await client.call("plan_document", ["source": .string(state.appendingPathComponent("card").path),
        "archive": .string("MEDIA"), "work": .string("ESSD"), "project": .string("roundtrip"), "cleanup": .bool(false)]).string
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

@Test func persistentReaderAnswersReadsInOrderAndRefusesMutations() async throws {
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let state = URL(fileURLWithPath: "/private/tmp").appendingPathComponent("media-reader-swift-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let reader = PersistentReader(configuration: .init(python: "/usr/bin/python3", script: repo.appendingPathComponent("nu9ve.py").path, stateDirectory: state.path))
    defer { reader.stop() }
    async let first = reader.call("snapshot", ["cards": .bool(false)])
    async let second = reader.call("suggest", ["source": .string("/does/not/exist")])
    let (snapshot, suggestion) = try await (first, second)
    #expect(snapshot["volumes"].array.isEmpty)
    #expect(snapshot["defaults"]["profile"].string == "e64")
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
@Test func diskIndexCrossesTheRealBoundary() async throws {
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let scratch = URL(fileURLWithPath: "/private/tmp")
    let state = scratch.appendingPathComponent(UUID().uuidString)
    let folder = scratch.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state); try? FileManager.default.removeItem(at: folder) }
    try FileManager.default.createDirectory(at: folder.appendingPathComponent("Music"), withIntermediateDirectories: true)
    try Data(count: 2 * 1024 * 1024).write(to: folder.appendingPathComponent("Music/song.wav"))
    let configuration = BackendConfiguration(python: "/usr/bin/python3", script: repo.appendingPathComponent("nu9ve.py").path, stateDirectory: state.path)
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

// MARK: Rust engine first, Python as fallback (PLAN «Motor en Rust · ruta», paso 7)

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()

/// A stand-in engine: it has `snapshot`, `jobs` and `recipe`; `snapshot` is not ported here, `jobs` fails as a method
/// would, `recipe` stops the process mid-request.
private let fakeEngine = #"""
#!/usr/bin/python3
import json, sys
for line in sys.stdin:
    request = json.loads(line)
    method = request['method']
    if method == 'engine_info':
        out = {'ok': True, 'data': {'engine': 'rust', 'ported': ['snapshot', 'jobs', 'recipe'],
                                    'once': ['publications', 'retitle_operations', 'discover'],
                                    'writes': ['defaults', 'plan_document', 'apply']}}
    elif method == 'defaults':
        out = {'ok': True, 'data': {'project': 'from-rust'}}
    elif method == 'plan_document':
        out = {'ok': False, 'error': {'code': 'not_ported', 'message': 'x'}}
    elif method == 'publications':
        out = {'ok': True, 'data': [{'asset_id': 'from-rust'}]}
    elif method == 'retitle_operations':
        out = {'ok': False, 'error': {'code': 'not_ported', 'message': 'x'}}
    elif method == 'snapshot':
        out = {'ok': False, 'error': {'code': 'not_ported', 'message': 'x'}}
    elif method == 'jobs':
        out = {'ok': False, 'error': {'code': 'unknown_project', 'message': 'Proyecto desconocido'}}
    else:
        sys.exit(3)
    out['id'] = request.get('id')
    sys.stdout.write(json.dumps(out) + '\n'); sys.stdout.flush()
"""#

@Test func readsAskRustFirstAndPythonAnswersWhatRustCannot() async throws {
    let scratch = URL(fileURLWithPath: "/private/tmp").appendingPathComponent("media-engine-swift-" + UUID().uuidString)
    try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: scratch) }
    let engine = scratch.appendingPathComponent("fake-engine")
    try fakeEngine.write(to: engine, atomically: true, encoding: .utf8)
    try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: engine.path)
    let reader = PersistentReader(configuration: .init(python: "/usr/bin/python3", script: repoRoot.appendingPathComponent("nu9ve.py").path,
                                                        stateDirectory: scratch.appendingPathComponent("state").path, engine: engine.path))
    defer { reader.stop() }
    // `not_ported` from Rust: Python answers.
    let snapshot = try await reader.call("snapshot", ["cards": .bool(false)])
    #expect(snapshot["defaults"]["profile"].string == "e64")
    #expect(reader.answeredBy["snapshot"] == "python")
    // An error of the method itself is Rust's answer, as Python's would be.
    do {
        _ = try await reader.call("jobs")
        Issue.record("Rust's method error was not returned")
    } catch let failure as BackendFailure { #expect(failure.code == "unknown_project") }
    #expect(reader.answeredBy["jobs"] == "rust")
    // Not among the engine's methods: straight to Python.
    let suggestion = try await reader.call("suggest", ["source": .string("/does/not/exist")])
    #expect(suggestion["reason"].string == "no_registered_destinations")
    #expect(reader.answeredBy["suggest"] == "python")
    // The engine stops mid-request: Python answers.
    let recipe = try await reader.call("recipe")
    #expect(recipe["cards"].array.first?["family"].string == "stalker")
    #expect(reader.answeredBy["recipe"] == "python")
}

@Test(.enabled(if: FileManager.default.isExecutableFile(atPath: repoRoot.appendingPathComponent("engine/target/release/nuun-engine").path)))
func theRealEngineAnswersReadsLikePython() async throws {
    let state = URL(fileURLWithPath: "/private/tmp").appendingPathComponent("media-rust-swift-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let base = BackendConfiguration(python: "/usr/bin/python3", script: repoRoot.appendingPathComponent("nu9ve.py").path, stateDirectory: state.path,
                                    engine: repoRoot.appendingPathComponent("engine/target/release/nuun-engine").path)
    var pythonOnly = base
    pythonOnly.reads = "python"
    let rust = PersistentReader(configuration: base), python = PersistentReader(configuration: pythonOnly)
    defer { rust.stop(); python.stop() }
    for (method, params) in [("snapshot", ["cards": JSONValue.bool(false)]), ("recipe", [:]), ("jobs", [:]), ("library", [:])] {
        var mine = try await rust.call(method, params), theirs = try await python.call(method, params)
        if method == "snapshot" {
            // Clipper and Stalker are this machine's, asked at different moments.
            if case .object(var a) = mine { a["capabilities"] = nil; mine = .object(a) }
            if case .object(var b) = theirs { b["capabilities"] = nil; theirs = .object(b) }
        }
        #expect(mine == theirs, "\(method)")
        #expect(rust.answeredBy[method] == "rust")
        #expect(python.answeredBy[method] == "python")
    }
}

@Test func oneShotReadsAskRustFirstAndNeverSendWrites() async throws {
    let scratch = URL(fileURLWithPath: "/private/tmp").appendingPathComponent("media-once-swift-" + UUID().uuidString)
    try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: scratch) }
    let engine = scratch.appendingPathComponent("fake-engine")
    try fakeEngine.write(to: engine, atomically: true, encoding: .utf8)
    try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: engine.path)
    let client = BackendClient(configuration: .init(python: "/usr/bin/python3", script: repoRoot.appendingPathComponent("nu9ve.py").path,
                                                    stateDirectory: scratch.appendingPathComponent("state").path, engine: engine.path))
    // Declared one-shot read: Rust answers.
    #expect(try await client.call("publications").array.first?["asset_id"].string == "from-rust")
    // `not_ported` from Rust: Python answers (an empty catalog has no operations).
    #expect(try await client.call("retitle_operations").array.isEmpty)
    // Not declared: Python answers, and its error is the answer.
    do {
        _ = try await client.call("resolve", ["asset_id": .string("nope")])
        Issue.record("resolve unexpectedly answered")
    } catch let failure as BackendFailure { #expect(failure.code == "unknown_asset") }
    // A write never reaches the engine, even with Rust reads on.
    let saved = try await client.call("defaults", ["project": .string("x")])
    #expect(saved["project"].string == "x")

    let sandbox = BackendClient(configuration: .init(python: "/usr/bin/python3", script: repoRoot.appendingPathComponent("nu9ve.py").path,
                                                     stateDirectory: scratch.appendingPathComponent("state").path, engine: engine.path, rustWrites: true))
    #expect(try await sandbox.call("defaults", ["project": .string("x")])["project"].string == "from-rust")
    do {
        _ = try await sandbox.call("plan_document")
        Issue.record("Python should reject a request without a source")
    } catch let failure as BackendFailure { #expect(failure.code != "not_ported") }
    do {
        _ = try await sandbox.call("apply")
        Issue.record("The fake engine terminates without an apply response")
    } catch let failure as BackendFailure { #expect(failure.code == "backend_failed") }
}
