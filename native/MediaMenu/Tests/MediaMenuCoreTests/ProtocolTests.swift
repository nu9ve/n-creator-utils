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
