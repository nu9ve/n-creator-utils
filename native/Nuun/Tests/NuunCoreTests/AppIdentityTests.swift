import Foundation
import NuMacShell
import Testing

// macos-app-v1 §5: About reads its facts from the bundle, and nüün's build script stamps them with the keys of
// NuMacShell's `AppIdentity` (nu9ve/core, F3 2026-11-26). The type's own behavior is tested in the core.

@Test func identityReadsTheStampedBundle() {
    let info: [String: Any] = ["CFBundleShortVersionString": "0.2.0", "CFBundleVersion": "7",
                               "Nu9veBuildID": "d220fa121c80", "Nu9veBuildDate": "2026-11-07T07:49:05+00:00",
                               "NSHumanReadableCopyright": "© 2026 nu9ve"]
    let identity = AppIdentity(name: "nüün", tagline: "x", info: info)
    #expect(identity.buildID == "d220fa121c80" && identity.buildDate != nil)
    #expect(identity.revisionLine?.hasPrefix("d220fa121c80 · ") == true)
    #expect(identity.copyright == "© 2026 nu9ve")
    let line = identity.diagnostics(system: OperatingSystemVersion(majorVersion: 26, minorVersion: 1, patchVersion: 0))
    #expect(line.hasPrefix("nüün 0.2.0 (7) d220fa121c80 · macOS 26.1.0 · "))
}

@Test func unstampedBundleHasNoRevision() {
    let identity = AppIdentity(name: "nüün", tagline: "x", info: ["CFBundleShortVersionString": "$(MARKETING_VERSION)", "Nu9veBuildID": " "])
    #expect(identity.buildID == nil && identity.revisionLine == nil)
}

@Test func buildScriptStampsTheSharedKeys() throws {
    // The installer's Info.plist and About must agree (macos-app-v1 §5).
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let script = try String(contentsOf: repo.appendingPathComponent("scripts/build_media_app.py"), encoding: .utf8)
    #expect(script.contains("'NSHumanReadableCopyright': '© 2026 nu9ve'"))
    #expect(script.contains("'\(AppIdentity.InfoKey.buildID)'") && script.contains("'\(AppIdentity.InfoKey.buildDate)'"))
}
